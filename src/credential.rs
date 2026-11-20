//! Looks up a username and password for a push from the host's git
//! configuration, following gitcredentials(7).
//!
//! git2's own helper lookup ignores URL-scoped settings such as
//! `credential.http://192.0.2.1.helper` when the host is an IP address, so the
//! lookup is done here: collect the helpers that apply to the URL, in
//! configuration order, and ask each one.

use std::io::Write;
use std::process::{Command, Stdio};

use git2::Config;
use url::Url;

/// What the configuration says about credentials for one URL.
#[derive(Debug, Default, PartialEq)]
struct Settings {
    /// Helper commands in the order git would try them.
    helpers: Vec<String>,
    username: Option<String>,
    use_http_path: bool,
}

/// Returns `(username, password)` for `url`, or `None` when no helper knows it.
pub fn lookup(config: &Config, url: &str, username: Option<&str>) -> Option<(String, String)> {
    let target = Url::parse(url).ok()?;
    let mut settings = read_settings(config, &target);
    if let Some(u) = username {
        settings.username = Some(u.to_string());
    }
    if settings.username.is_none() && !target.username().is_empty() {
        settings.username = Some(target.username().to_string());
    }

    let mut username = settings.username.clone();
    let mut password = None;
    for helper in &settings.helpers {
        let (u, p) = ask(helper, &target, username.as_deref(), settings.use_http_path);
        if username.is_none() {
            username = u;
        }
        if password.is_none() {
            password = p;
        }
        if username.is_some() && password.is_some() {
            break;
        }
    }
    Some((username?, password?))
}

fn read_settings(config: &Config, target: &Url) -> Settings {
    let mut settings = Settings::default();
    let Ok(mut entries) = config.entries(Some("^credential\\.")) else {
        return settings;
    };
    while let Some(entry) = entries.next() {
        let Ok(entry) = entry else { continue };
        let (Ok(name), value) = (entry.name(), entry.value().unwrap_or("")) else {
            continue;
        };
        // `credential.<key>` or `credential.<url pattern>.<key>`; keys hold no dots.
        let Some(rest) = name.strip_prefix("credential.") else {
            continue;
        };
        let (pattern, key) = match rest.rsplit_once('.') {
            Some((pattern, key)) => (Some(pattern), key),
            None => (None, rest),
        };
        if let Some(pattern) = pattern {
            if !pattern_matches(pattern, target) {
                continue;
            }
        }
        match key.to_ascii_lowercase().as_str() {
            // An empty value clears the helpers collected so far.
            "helper" if value.is_empty() => settings.helpers.clear(),
            "helper" => settings.helpers.push(helper_command(value)),
            "username" => settings.username = Some(value.to_string()),
            "usehttppath" => settings.use_http_path = matches!(value, "" | "true" | "yes" | "on" | "1"),
            _ => {}
        }
    }
    settings
}

/// Whether a `credential.<pattern>` section applies to `target`.
fn pattern_matches(pattern: &str, target: &Url) -> bool {
    let Ok(p) = Url::parse(pattern) else {
        return false;
    };
    if p.scheme() != target.scheme() {
        return false;
    }
    let (Some(p_host), Some(t_host)) = (p.host_str(), target.host_str()) else {
        return false;
    };
    let host_ok = match p_host.strip_prefix("*.") {
        Some(suffix) => t_host
            .strip_suffix(suffix)
            .is_some_and(|head| head.ends_with('.') && head.len() > 1),
        None => p_host.eq_ignore_ascii_case(t_host),
    };
    if !host_ok {
        return false;
    }
    if p.port().is_some() && p.port_or_known_default() != target.port_or_known_default() {
        return false;
    }
    if !p.username().is_empty() && p.username() != target.username() {
        return false;
    }
    let p_path = p.path().trim_end_matches('/');
    if p_path.is_empty() {
        return true;
    }
    let t_path = target.path();
    t_path == p_path || t_path.strip_prefix(p_path).is_some_and(|rest| rest.starts_with('/'))
}

/// Turns a `helper` value into the shell command to run (without the action).
fn helper_command(value: &str) -> String {
    if let Some(shell) = value.strip_prefix('!') {
        shell.to_string()
    } else if value.starts_with('/') || value.starts_with('\\') || value.chars().nth(1) == Some(':') {
        value.to_string()
    } else {
        format!("git credential-{value}")
    }
}

/// Runs one helper with the `get` action and reads its answer.
fn ask(helper: &str, target: &Url, username: Option<&str>, use_http_path: bool) -> (Option<String>, Option<String>) {
    let mut request = format!("protocol={}\n", target.scheme());
    if let Some(host) = target.host_str() {
        match target.port() {
            Some(port) => request.push_str(&format!("host={host}:{port}\n")),
            None => request.push_str(&format!("host={host}\n")),
        }
    }
    if use_http_path {
        request.push_str(&format!("path={}\n", target.path().trim_start_matches('/')));
    }
    if let Some(u) = username {
        request.push_str(&format!("username={u}\n"));
    }
    request.push('\n');

    let spawned = Command::new("sh")
        .arg("-c")
        .arg(format!("{helper} get"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        return (None, None);
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(request.as_bytes());
    }
    let Ok(output) = child.wait_with_output() else {
        return (None, None);
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
            .map(str::to_string)
    };
    (field("username"), field("password"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_from(text: &str) -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, text).unwrap();
        (dir, Config::open(&path).unwrap())
    }

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn url_scoped_helper_with_ip_host_and_reset_line_is_used() {
        // The shape that failed in v0.1.0: IP host, an empty reset line, then the helper.
        let (_dir, config) = config_from(
            "[credential]\n\thelper = osxkeychain\n\
             [credential \"http://192.0.2.10\"]\n\thelper =\n\thelper = store --file /tmp/creds\n",
        );
        let s = read_settings(&config, &url("http://192.0.2.10/gitea/org/repo.git"));
        assert_eq!(s.helpers, vec!["git credential-store --file /tmp/creds".to_string()]);
    }

    #[test]
    fn unrelated_url_section_is_ignored() {
        let (_dir, config) = config_from(
            "[credential]\n\thelper = cache\n[credential \"http://192.0.2.10\"]\n\thelper = store\n",
        );
        let s = read_settings(&config, &url("http://192.0.2.99/repo.git"));
        assert_eq!(s.helpers, vec!["git credential-cache".to_string()]);
    }

    #[test]
    fn pattern_matching_follows_scheme_host_port_and_path() {
        let t = url("https://git.example.com:8443/team/repo.git");
        assert!(pattern_matches("https://git.example.com", &t));
        assert!(pattern_matches("https://git.example.com:8443", &t));
        assert!(pattern_matches("https://git.example.com/team", &t));
        assert!(pattern_matches("https://*.example.com", &t));
        assert!(!pattern_matches("http://git.example.com", &t));
        assert!(!pattern_matches("https://git.example.com:9000", &t));
        assert!(!pattern_matches("https://git.example.com/other", &t));
        assert!(!pattern_matches("https://example.com", &t));
        assert!(!pattern_matches("https://*.example.org", &t));
    }

    #[test]
    fn helper_value_forms() {
        assert_eq!(helper_command("store --file x"), "git credential-store --file x");
        assert_eq!(helper_command("!f() { echo hi; }; f"), "f() { echo hi; }; f");
        assert_eq!(helper_command("/usr/local/bin/helper"), "/usr/local/bin/helper");
    }

    #[cfg(unix)]
    #[test]
    fn lookup_runs_the_url_scoped_helper() {
        let (_dir, config) = config_from(
            "[credential \"http://192.0.2.10\"]\n\thelper =\n\
             \thelper = \"!f() { cat >/dev/null; echo username=alice; echo password=s3cret; }; f\"\n",
        );
        let found = lookup(&config, "http://192.0.2.10/gitea/org/repo.git", None);
        assert_eq!(found, Some(("alice".to_string(), "s3cret".to_string())));
        assert_eq!(lookup(&config, "http://192.0.2.99/repo.git", None), None);
    }

    #[cfg(unix)]
    #[test]
    fn username_from_config_is_sent_to_the_helper() {
        let (_dir, config) = config_from(
            "[credential \"http://192.0.2.10\"]\n\tusername = bob\n\
             \thelper = \"!f() { grep -q '^username=bob$' && echo password=pw; }; f\"\n",
        );
        let found = lookup(&config, "http://192.0.2.10/repo.git", None);
        assert_eq!(found, Some(("bob".to_string(), "pw".to_string())));
    }
}
