//! CLI contract for external agents (see AGENTS.md): exit codes, stdout, --db handling

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Fresh empty directory per test, removed on drop; cwd is set there so no project .env is picked up
struct TempDir(PathBuf);

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir() -> TempDir {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("majowuji-cli-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_majowuji"));
    cmd.args(args)
        .current_dir(dir)
        .env_remove("MAJOWUJI_DB")
        .env_remove("RUST_LOG");
    cmd
}

fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}

fn log(dir: &Path, db: &str, exercise: &str) -> serde_json::Value {
    let out = run(dir, &["--db", db, "log", exercise, "-s", "2", "-r", "5", "--create", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

fn assert_failed_clean(out: &Output) {
    assert_eq!(out.status.code(), Some(1), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "stdout must be empty on error");
}

#[test]
fn list_json_is_newest_first() {
    let dir = temp_dir();
    for ex in ["a", "b", "c"] {
        log(&dir, "t.db", ex);
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<i64> = list.iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, vec![3, 2, 1]);
}

#[test]
fn read_commands_fail_on_non_database() {
    let dir = temp_dir();
    std::fs::write(dir.join("text.db"), "not a database at all, just some text\n").unwrap();
    std::fs::create_dir(dir.join("subdir")).unwrap();
    for db in ["text.db", "subdir"] {
        for cmd in [&["list", "--json"][..], &["stats", "--json"][..]] {
            let mut args = vec!["--db", db];
            args.extend_from_slice(cmd);
            assert_failed_clean(&run(&dir, &args));
        }
    }
}

#[test]
fn read_commands_fail_on_empty_file_and_leave_it_empty() {
    let dir = temp_dir();
    std::fs::write(dir.join("empty.db"), "").unwrap();
    assert_failed_clean(&run(&dir, &["--db", "empty.db", "list", "--json"]));
    assert_failed_clean(&run(&dir, &["--db", "empty.db", "stats", "--json"]));
    assert_eq!(std::fs::metadata(dir.join("empty.db")).unwrap().len(), 0);
}

#[test]
fn read_commands_do_not_create_missing_database() {
    let dir = temp_dir();
    assert_failed_clean(&run(&dir, &["--db", "missing.db", "list", "--json"]));
    assert!(!dir.join("missing.db").exists());
}

#[test]
fn read_commands_do_not_modify_database() {
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    let before = std::fs::read(dir.join("t.db")).unwrap();
    assert_eq!(run(&dir, &["--db", "t.db", "list", "--json"]).status.code(), Some(0));
    assert_eq!(run(&dir, &["--db", "t.db", "stats", "jab", "--json"]).status.code(), Some(0));
    assert_eq!(std::fs::read(dir.join("t.db")).unwrap(), before);
}

#[test]
fn in_memory_database_is_rejected() {
    let dir = temp_dir();
    for db in ["", ":memory:", "file::memory:"] {
        assert_failed_clean(&run(&dir, &["--db", db, "log", "jab", "--json"]));
    }
}

#[test]
fn log_rejects_invalid_input() {
    let dir = temp_dir();
    for args in [
        &["-s", "0"][..],
        &["-r", "0"][..],
        &["-r", "-3"][..],
        &["--duration", "0"][..],
        &["--duration", "-5"][..],
        &["--pulse-before", "0"][..],
        &["--pulse-before", "29"][..],
        &["--pulse-after", "300"][..],
    ] {
        let mut full = vec!["--db", "t.db", "log", "jab"];
        full.extend_from_slice(args);
        let out = run(&dir, &full);
        assert_eq!(out.status.code(), Some(2), "args {:?}", args);
        assert!(out.stdout.is_empty());
    }
    assert_failed_clean(&run(&dir, &["--db", "t.db", "log", "   ", "--json"]));
}

#[test]
fn log_writes_duration_and_pulse() {
    let dir = temp_dir();
    let out = run(&dir, &["--db", "t.db", "log", "plank", "-s", "1", "-r", "1", "--create",
        "--duration", "67", "--pulse-before", "76", "--pulse-after", "94", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let logged: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(logged["duration_secs"], 67);
    assert_eq!(logged["pulse_before"], 76);
    assert_eq!(logged["pulse_after"], 94);

    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list[0]["duration_secs"], 67);
    assert_eq!(list[0]["pulse_before"], 76);
    assert_eq!(list[0]["pulse_after"], 94);
}

#[test]
fn write_commands_do_not_create_missing_database() {
    let dir = temp_dir();
    assert_failed_clean(&run(&dir, &["--db", "missing.db", "log", "jab"]));
    assert_failed_clean(&run(&dir, &["--db", "missing.db", "intervals", "sync", "--athlete", "i1"]));
    assert!(!dir.join("missing.db").exists());
}

#[test]
fn log_create_initializes_new_database() {
    let dir = temp_dir();
    let out = run(&dir, &["--db", "fresh.db", "log", "jab", "--create", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let logged: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(logged["id"], 1);

    let out = run(&dir, &["--db", "fresh.db", "list", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["exercise"], "jab");
}

#[test]
fn stats_json_shapes() {
    let dir = temp_dir();
    log(&dir, "t.db", "Jab");
    let out = run(&dir, &["--db", "t.db", "stats", "jab", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["total_volume"], 10);
    assert_eq!(v["suggested_next"]["sets"], 2);
    let out = run(&dir, &["--db", "t.db", "stats", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["total_trainings"], 1);
}

#[test]
fn env_variable_selects_database() {
    let dir = temp_dir();
    log(&dir, "env.db", "jab");
    let out = command(&dir, &["list", "--json"]).env("MAJOWUJI_DB", "env.db").output().unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.len(), 1);
    assert!(!dir.join("majowuji.db").exists(), "default path must not be used");
}

#[test]
fn log_json_returns_stored_record() {
    let dir = temp_dir();
    let out = run(&dir, &["--db", "t.db", "log", "jab", "-s", "3", "-r", "7", "-n", "hip rotation", "--create", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let logged: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(logged["id"], 1);
    assert_eq!(logged["sets"], 3);
    assert_eq!(logged["reps"], 7);
    assert_eq!(logged["notes"], "hip rotation");
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list[0]["id"], 1);
    assert_eq!(list[0]["notes"], "hip rotation");
}

#[test]
fn list_respects_limit() {
    let dir = temp_dir();
    for ex in ["a", "b", "c"] {
        log(&dir, "t.db", ex);
    }
    let out = run(&dir, &["--db", "t.db", "list", "-l", "2", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.len(), 2);
}

#[test]
fn old_schema_needs_explicit_migrate_without_fake_records() {
    let dir = temp_dir();
    {
        let conn = rusqlite::Connection::open(dir.join("old.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE trainings (id INTEGER PRIMARY KEY AUTOINCREMENT, date TEXT NOT NULL, \
             exercise TEXT NOT NULL, sets INTEGER NOT NULL, reps INTEGER NOT NULL, notes TEXT);
             INSERT INTO trainings (date, exercise, sets, reps) VALUES ('2026-01-05 12:00:00', 'jab', 3, 50);",
        )
        .unwrap();
    }
    let out = run(&dir, &["--db", "old.db", "list", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));

    let out = run(&dir, &["--db", "old.db", "migrate"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let out = run(&dir, &["--db", "old.db", "list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.len(), 1, "migrate must not add records");
    assert_eq!(list[0]["exercise"], "jab");
}

#[test]
fn json_contract_details() {
    let dir = temp_dir();
    // list default limit is 10
    for i in 0..11 {
        log(&dir, "t.db", &format!("ex{}", i));
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list.len(), 10);

    // log --json returns exactly the stored date
    let logged = log(&dir, "t.db", "jab");
    let out = run(&dir, &["--db", "t.db", "list", "-l", "1", "--json"]);
    let list: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(list[0]["id"], logged["id"]);
    assert_eq!(list[0]["date"], logged["date"]);

    // stats keys, exactly as documented
    let out = run(&dir, &["--db", "t.db", "stats", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["total_trainings", "weekly_frequency"]);

    let out = run(&dir, &["--db", "t.db", "stats", "nothing-like-this", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["exercise", "suggested_next", "total_volume"]);
    assert_eq!(v["exercise"], "nothing-like-this");
    assert!(v["suggested_next"].is_null());
    assert_eq!(v["total_volume"], 0);
}

#[test]
fn migrate_requires_existing_database_and_rejects_json() {
    let dir = temp_dir();
    assert_failed_clean(&run(&dir, &["--db", "missing.db", "migrate"]));
    assert!(!dir.join("missing.db").exists());
    std::fs::write(dir.join("empty.db"), "").unwrap();
    assert_failed_clean(&run(&dir, &["--db", "empty.db", "migrate"]));
    log(&dir, "t.db", "jab");
    assert_failed_clean(&run(&dir, &["--db", "t.db", "migrate", "--json"]));
}

#[test]
fn missing_watch_sessions_requires_migrate() {
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch("DROP TABLE watch_sessions;").unwrap();
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));

    let out = run(&dir, &["--db", "t.db", "migrate"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
}
