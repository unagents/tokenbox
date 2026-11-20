//! End-to-end checks against a local bare repository used as the store.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

struct Env {
    _root: TempDir,
    store: String,
    scratch: std::path::PathBuf,
}

fn setup() -> Env {
    let root = TempDir::new().unwrap();
    let store = root.path().join("store.git");
    git2::Repository::init_bare(&store).unwrap();
    let scratch = root.path().join("scratch");
    std::fs::create_dir(&scratch).unwrap();
    Env { store: store.to_str().unwrap().to_string(), scratch, _root: root }
}

fn run(env: &Env, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tokenbox"))
        .args(args)
        // The temporary repository must land in a directory the test can inspect.
        .env("TMPDIR", &env.scratch)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(text) = stdin {
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

fn is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn register_then_get_round_trips_and_leaves_nothing_behind() {
    let env = setup();
    let key = "service-token-e2e-7f3a-91c2";

    let out = run(&env, &[&env.store, "register", key, "--stdin"], Some("first-value\n"));
    assert_eq!(code(&out), 0, "register failed: {}", String::from_utf8_lossy(&out.stderr));

    let out = run(&env, &[&env.store, "get", key], None);
    assert_eq!(code(&out), 0, "get failed: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "first-value\n");

    // Registering again replaces the value.
    let out = run(&env, &[&env.store, "register", key, "--stdin"], Some("second-value"));
    assert_eq!(code(&out), 0, "re-register failed: {}", String::from_utf8_lossy(&out.stderr));
    let out = run(&env, &[&env.store, "get", key, "--json"], None);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "{\"value\":\"second-value\"}\n");

    assert!(is_empty(&env.scratch), "a temporary repository was left behind");
}

#[test]
fn wrong_key_name_exits_with_2() {
    let env = setup();
    let out = run(&env, &[&env.store, "register", "right-name-0001", "--stdin"], Some("value"));
    assert_eq!(code(&out), 0);

    let out = run(&env, &[&env.store, "get", "wrong-name-0001"], None);
    assert_eq!(code(&out), 2);
    assert!(out.stdout.is_empty());
}

#[test]
fn get_from_an_empty_store_exits_with_2() {
    let env = setup();
    let out = run(&env, &[&env.store, "get", "anything"], None);
    assert_eq!(code(&out), 2);
}

#[test]
fn unreachable_store_exits_with_3() {
    let env = setup();
    let missing = format!("{}-does-not-exist", env.store);
    let out = run(&env, &[&missing, "get", "anything"], None);
    assert_eq!(code(&out), 3);
    assert!(is_empty(&env.scratch));
}

#[test]
fn usage_errors_exit_with_1() {
    let env = setup();
    assert_eq!(code(&run(&env, &[], None)), 1);
    assert_eq!(code(&run(&env, &[&env.store, "register", "name", "--stdin"], Some(""))), 1);
    assert_eq!(code(&run(&env, &["--version"], None)), 0);
}

#[test]
fn store_reveals_neither_key_name_nor_value() {
    let env = setup();
    let key = "plainly-visible-key-name";
    let value = "plainly-visible-value";
    let out = run(&env, &[&env.store, "register", key, "--stdin"], Some(value));
    assert_eq!(code(&out), 0);

    let repo = git2::Repository::open_bare(&env.store).unwrap();
    let odb = repo.odb().unwrap();
    let mut seen = Vec::new();
    odb.foreach(|oid| {
        seen.push(*oid);
        true
    })
    .unwrap();
    assert!(!seen.is_empty());
    for oid in seen {
        let obj = odb.read(oid).unwrap();
        let text = String::from_utf8_lossy(obj.data()).to_string();
        assert!(!text.contains(key), "key name found in object {oid}");
        assert!(!text.contains(value), "value found in object {oid}");
    }
}
