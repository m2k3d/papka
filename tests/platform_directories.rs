#![cfg(target_os = "linux")]

#[path = "../src/platform.rs"]
#[allow(dead_code)] // В Linux-сценариях вариант UnsupportedPlatform недостижим.
mod platform;

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "papka-paths-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn check(config: Option<OsString>, state: Option<OsString>, home_present: bool) {
    let sandbox = Sandbox::new();
    let home = sandbox.0.join("home");
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--ignored", "--exact", "probe", "--nocapture"])
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("HOME");
    if home_present {
        child.env("HOME", &home);
    }
    let expected_home = if home_present {
        home
    } else {
        // Linux использует запись пользователя в passwd, если HOME отсутствует.
        let uid = std::fs::metadata("/proc/self").unwrap().uid().to_string();
        let output = Command::new("getent")
            .args(["passwd", &uid])
            .output().unwrap();
        assert!(output.status.success());
        let record = String::from_utf8(output.stdout).unwrap();
        PathBuf::from(record.split(':').nth(5).unwrap())
    };
    let mut expected_config = expected_home.join(".config");
    let mut expected_state = expected_home.join(".local/state");
    if let Some(value) = config {
        let path = PathBuf::from(&value);
        if path.is_absolute() { expected_config = path; }
        child.env("XDG_CONFIG_HOME", value);
    }
    if let Some(value) = state {
        let path = PathBuf::from(&value);
        if path.is_absolute() { expected_state = path; }
        child.env("XDG_STATE_HOME", value);
    }
    child.env("PAPKA_EXPECT_CONFIG", expected_config.join("papka"))
        .env("PAPKA_EXPECT_STATE", expected_state.join("papka"));
    let output = child.output().unwrap();
    assert!(output.status.success(), "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr));
    // Даже заданный несуществующий HOME не должен появиться на диске.
    assert_eq!(std::fs::read_dir(&sandbox.0).unwrap().count(), 0);
}

#[test]
fn default_paths() { check(None, None, true); }

#[test]
fn absolute_xdg_paths() {
    let root = Sandbox::new();
    check(Some(root.0.join("config").into()), Some(root.0.join("state").into()), true);
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn empty_xdg_falls_back() { check(Some("".into()), Some("".into()), true); }

#[test]
fn relative_xdg_falls_back() {
    check(Some("relative-config".into()), Some("relative-state".into()), true);
}

#[test]
fn config_and_state_are_independent() {
    let root = Sandbox::new();
    check(Some(root.0.join("config").into()), None, true);
    check(None, Some(root.0.join("state").into()), true);
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn non_utf8_xdg_paths_are_preserved() {
    let root = Sandbox::new();
    let path = root.0.join(OsString::from_vec(b"data-\xff".to_vec()));
    check(Some(path.clone().into()), Some(path.into()), true);
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn missing_home_uses_system_home() { check(None, None, false); }

// Каждый сценарий запускает отдельный процесс: окружение тестового runner не меняется.
#[test]
#[ignore]
fn probe() {
    let paths = platform::AppDirectories::resolve().unwrap();
    assert_eq!(paths.config, PathBuf::from(std::env::var_os("PAPKA_EXPECT_CONFIG").unwrap()));
    assert_eq!(paths.history, PathBuf::from(std::env::var_os("PAPKA_EXPECT_STATE").unwrap()));
}
