//! Talks to the store over the git protocol with libgit2 (SPEC §5, §6).
//!
//! Every call works in a temporary bare repository that is removed when the
//! call returns, so nothing about the store stays on the host. Reads never
//! send credentials. Only the push in `write_file` asks the host's git
//! configuration for credentials.

use std::cell::Cell;
use std::path::Path;

use git2::{
    Cred, CredentialType, Direction, ErrorCode, FetchOptions, Oid, PushOptions, RemoteCallbacks,
    Repository, Signature,
};
use tempfile::TempDir;

use crate::error::{Failure, Result};

const BASE_REF: &str = "refs/tokenbox/base";
const PUSH_REF: &str = "refs/tokenbox/push";
const FALLBACK_BRANCH: &str = "refs/heads/main";
const PUSH_ATTEMPTS: usize = 3;

/// A temporary bare repository holding the newest commit of the store.
struct Checkout {
    // Dropped last: removes the directory.
    _dir: TempDir,
    repo: Repository,
    /// Branch the store uses, e.g. `refs/heads/main`.
    branch: String,
    /// Newest commit of that branch. `None` when the store has no commits yet.
    base: Option<Oid>,
}

/// Reads one file from the newest commit of the store.
/// Returns `None` when the file does not exist.
pub fn read_file(url: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let checkout = fetch(url)?;
    read_from(&checkout, path)
}

/// Writes one file to the store as a new commit and pushes it.
/// Returns the id of the pushed commit.
pub fn write_file(url: &str, path: &str, data: &[u8]) -> Result<String> {
    let mut last = None;
    for _ in 0..PUSH_ATTEMPTS {
        let checkout = fetch(url)?;
        let commit = commit_file(&checkout, path, data)?;
        match push(&checkout, url, commit) {
            Ok(()) => return Ok(commit.to_string()),
            // Someone else pushed first: fetch again and redo the commit on top.
            Err(PushError::Rejected(m)) => last = Some(m),
            Err(PushError::Fatal(f)) => return Err(f),
        }
    }
    Err(Failure::Store(format!(
        "push was rejected {PUSH_ATTEMPTS} times: {}",
        last.unwrap_or_default()
    )))
}

fn fetch(url: &str) -> Result<Checkout> {
    let dir = tempfile::Builder::new()
        .prefix("tokenbox-")
        .tempdir()
        .map_err(|e| Failure::Store(format!("cannot create a temporary directory: {e}")))?;
    let repo = Repository::init_bare(dir.path()).map_err(|e| store_err("cannot prepare a temporary repository", &e))?;

    let asked_for_auth = Cell::new(false);
    let result = fetch_into(&repo, url, &asked_for_auth);
    let (branch, base) = match result {
        Ok(v) => v,
        Err(e) if asked_for_auth.get() => {
            return Err(Failure::AuthRequired(format!(
                "the store asked for authentication. The store must be public; \
                 the git URL may also be wrong or the repository may not exist. ({})",
                e.message()
            )))
        }
        Err(e) => return Err(store_err("cannot fetch from the store", &e)),
    };
    Ok(Checkout { _dir: dir, repo, branch, base })
}

fn fetch_into(
    repo: &Repository,
    url: &str,
    asked_for_auth: &Cell<bool>,
) -> std::result::Result<(String, Option<Oid>), git2::Error> {
    // Reads are unauthenticated: a request for credentials is an error.
    let no_credentials = || {
        let mut cb = RemoteCallbacks::new();
        cb.credentials(|_, _, _| {
            asked_for_auth.set(true);
            Err(git2::Error::from_str("authentication is not used for reads"))
        });
        cb
    };

    let mut remote = repo.remote_anonymous(url)?;
    let branch = {
        let conn = remote.connect_auth(Direction::Fetch, Some(no_credentials()), None)?;
        let heads: Vec<String> = conn
            .list()?
            .iter()
            .map(|h| h.name().to_string())
            .filter(|n| n.starts_with("refs/heads/"))
            .collect();
        if heads.is_empty() {
            return Ok((FALLBACK_BRANCH.to_string(), None));
        }
        let default = conn
            .default_branch()
            .ok()
            .and_then(|b| b.as_str().ok().map(str::to_string))
            .filter(|b| heads.contains(b));
        match default {
            Some(b) => b,
            None if heads.iter().any(|h| h == FALLBACK_BRANCH) => FALLBACK_BRANCH.to_string(),
            None => heads[0].clone(),
        }
    };

    let refspec = format!("+{branch}:{BASE_REF}");
    let mut shallow = FetchOptions::new();
    shallow.remote_callbacks(no_credentials());
    shallow.depth(1);
    if remote.fetch(&[&refspec], Some(&mut shallow), None).is_err() && !asked_for_auth.get() {
        // Some transports cannot do a shallow fetch; fall back to a full one.
        let mut full = FetchOptions::new();
        full.remote_callbacks(no_credentials());
        remote.fetch(&[&refspec], Some(&mut full), None)?;
    }
    let base = repo.find_reference(BASE_REF)?.peel_to_commit()?.id();
    Ok((branch, Some(base)))
}

fn read_from(checkout: &Checkout, path: &str) -> Result<Option<Vec<u8>>> {
    let Some(base) = checkout.base else {
        return Ok(None);
    };
    let read = || -> std::result::Result<Option<Vec<u8>>, git2::Error> {
        let tree = checkout.repo.find_commit(base)?.tree()?;
        match tree.get_path(Path::new(path)) {
            Ok(entry) => Ok(Some(checkout.repo.find_blob(entry.id())?.content().to_vec())),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    };
    read().map_err(|e| store_err("cannot read from the store", &e))
}

/// Builds a commit that adds or replaces `dir/name` on top of the fetched commit.
fn commit_file(checkout: &Checkout, path: &str, data: &[u8]) -> Result<Oid> {
    let (dir, name) = path
        .split_once('/')
        .ok_or_else(|| Failure::Store(format!("unexpected store path: {path}")))?;
    let repo = &checkout.repo;
    let build = || -> std::result::Result<Oid, git2::Error> {
        let parent = match checkout.base {
            Some(id) => Some(repo.find_commit(id)?),
            None => None,
        };
        let root = match &parent {
            Some(c) => Some(c.tree()?),
            None => None,
        };
        let sub = match &root {
            Some(t) => match t.get_name(dir) {
                Some(entry) => Some(repo.find_tree(entry.id())?),
                None => None,
            },
            None => None,
        };

        let blob = repo.blob(data)?;
        let mut sub_builder = repo.treebuilder(sub.as_ref())?;
        sub_builder.insert(name, blob, 0o100644)?;
        let sub_id = sub_builder.write()?;

        let mut root_builder = repo.treebuilder(root.as_ref())?;
        root_builder.insert(dir, sub_id, 0o040000)?;
        let tree = repo.find_tree(root_builder.write()?)?;

        let sig = repo
            .signature()
            .or_else(|_| Signature::now("tokenbox", "tokenbox@localhost"))?;
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        // The message must not reveal the key name.
        repo.commit(Some(PUSH_REF), &sig, &sig, "tokenbox: register", &tree, &parents)
    };
    // A leftover ref from an earlier attempt in the same repository cannot exist:
    // every attempt uses a fresh temporary repository.
    build().map_err(|e| store_err("cannot build the commit", &e))
}

enum PushError {
    /// The remote moved on; retrying on top of the new commit can succeed.
    Rejected(String),
    Fatal(Failure),
}

fn push(checkout: &Checkout, url: &str, commit: Oid) -> std::result::Result<(), PushError> {
    let fatal = |what: &str, e: &git2::Error| PushError::Fatal(store_err(what, e));
    let config = git2::Config::open_default().map_err(|e| fatal("cannot open the git configuration", &e))?;
    let tries = Cell::new(0u32);
    let rejected: Cell<Option<String>> = Cell::new(None);

    let mut cb = RemoteCallbacks::new();
    cb.credentials(|url, username, allowed| {
        // libgit2 asks again after a failed attempt; stop instead of looping.
        tries.set(tries.get() + 1);
        if tries.get() > 2 {
            return Err(git2::Error::from_str("the credentials from the git configuration were not accepted"));
        }
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            Cred::credential_helper(&config, url, username)
        } else if allowed.contains(CredentialType::SSH_KEY) {
            Cred::ssh_key_from_agent(username.unwrap_or("git"))
        } else if allowed.contains(CredentialType::DEFAULT) {
            Cred::default()
        } else {
            Err(git2::Error::from_str("no usable credential type"))
        }
    });
    cb.push_update_reference(|_, status| {
        if let Some(msg) = status {
            rejected.set(Some(msg.to_string()));
        }
        Ok(())
    });

    let mut opts = PushOptions::new();
    opts.remote_callbacks(cb);
    let mut remote = checkout
        .repo
        .remote_anonymous(url)
        .map_err(|e| fatal("cannot prepare the push", &e))?;
    let refspec = format!("{PUSH_REF}:{}", checkout.branch);
    debug_assert_eq!(checkout.repo.refname_to_id(PUSH_REF).ok(), Some(commit));

    match remote.push(&[&refspec], Some(&mut opts)) {
        Ok(()) => match rejected.take() {
            Some(msg) => Err(PushError::Rejected(msg)),
            None => Ok(()),
        },
        Err(e) if e.code() == ErrorCode::NotFastForward => Err(PushError::Rejected(e.message().to_string())),
        Err(e) => Err(fatal(
            "cannot push to the store (pushing needs credentials in this host's git configuration)",
            &e,
        )),
    }
}

fn store_err(what: &str, e: &git2::Error) -> Failure {
    Failure::Store(format!("{what}: {}", e.message()))
}
