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
    // The empty-name check lives after the missing-database check (main.rs), so it
    // is only reachable on a valid database — otherwise this control proves the
    // wrong guard
    log(&dir, "t.db", "jab");
    let out = run(&dir, &["--db", "t.db", "log", "   ", "--json"]);
    assert_failed_clean(&out);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("exercise name must not be empty"),
        "stderr: {}", String::from_utf8_lossy(&out.stderr)
    );
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

#[test]
fn migrate_completes_partially_pulsed_database() {
    // A file interrupted between the two pulse ALTERs of an older migration has
    // only pulse_before; migrate must still add pulse_after (r1 finding, P2)
    let dir = temp_dir();
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER UNIQUE NOT NULL,
                username TEXT,
                first_name TEXT,
                created_at TEXT NOT NULL,
                is_owner BOOLEAN DEFAULT FALSE);
             CREATE TABLE trainings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                exercise TEXT NOT NULL,
                sets INTEGER NOT NULL,
                reps INTEGER NOT NULL,
                duration_secs INTEGER,
                pulse_before INTEGER,
                notes TEXT,
                user_id INTEGER REFERENCES users(id));
             INSERT INTO trainings (date, exercise, sets, reps, pulse_before)
             VALUES ('2026-10-01T18:00:00+00:00', 'jab', 1, 6, 90);",
        ).unwrap();
    }
    let out = run(&dir, &["--db", "t.db", "migrate"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let conn = rusqlite::Connection::open_with_flags(
        dir.join("t.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let kept: i64 = conn.query_row(
        "SELECT COUNT(*) FROM trainings WHERE pulse_before = 90", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, 1, "existing pulse value must survive migration");
    // schema_is_current selects pulse_after: an error here means the column is missing
    let after: i64 = conn.query_row(
        "SELECT COUNT(*) FROM trainings WHERE pulse_after IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(after, 1);
}

#[test]
fn intervals_sync_cli_is_idempotent_and_fills_once() {
    use std::io::{Read as _, Write as _};

    let dir = temp_dir();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // the set is logged while the watch workout is still running (within its window)
    let start_dt = chrono::Utc::now() - chrono::Duration::seconds(60);
    let start = start_dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let activities = format!(
        r#"[{{"id":"a1","start_date":"{start}","type":"WeightTraining","elapsed_time":300,"average_heartrate":98.4,"max_heartrate":119.0,"calories":11.0,"source":"ZEPP"}}]"#);
    let streams = r#"[{"type":"time","data":[0,19,40,70]},{"type":"heartrate","data":[0,98,100,100]}]"#;
    std::thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let mut req = Vec::new();
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                }
                if req.windows(4).any(|w| w == b"\r\n\r\n") { break; }
            }
            let text = String::from_utf8_lossy(&req);
            let body: &str = if text.contains("/activities") { &activities } else { &streams };
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body);
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let logged = log(&dir, "t.db", "отжимания");
    let training_id = logged["id"].as_i64().unwrap();

    // A bot-user training inside the same window (r2 finding): sync belongs to
    // the CLI/owner record (user_id NULL) and must never touch another user's row
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute(
            "INSERT INTO users (chat_id, username, first_name, created_at, is_owner)
             VALUES (42, 'other', NULL, '2026-01-01T00:00:00+00:00', FALSE)",
            [],
        ).unwrap();
        let user_id: i64 = conn.last_insert_rowid();
        // fixed relative to the session start (after the 40 s HR sample, inside the
        // window): with the user_id filter removed (mutant) this row ALWAYS links,
        // so the negative control does not depend on clock-second boundaries
        let foreign = (start_dt + chrono::Duration::seconds(45))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        conn.execute(
            "INSERT INTO trainings (date, exercise, sets, reps, user_id)
             VALUES (?1, 'приседания', 1, 17, ?2)",
            rusqlite::params![foreign, user_id],
        ).unwrap();
    }

    // the full CLI path: env key, mock base override, two consecutive syncs
    let base = format!("http://{}", addr);
    let sync = || {
        command(&dir, &["--db", "t.db", "intervals", "sync", "--athlete", "i1", "--json"])
            .env("INTERVALS_API_KEY", "test-key")
            .env("MAJOWUJI_INTERVALS_API_BASE", &base)
            .output().unwrap()
    };
    let first = sync();
    assert_eq!(first.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&first.stderr));
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["sessions"], 1);
    assert_eq!(report["filled"].as_array().unwrap().len(), 1);
    assert_eq!(report["filled"][0]["training_id"], training_id);
    assert_eq!(report["filled"][0]["session_id"], "a1");
    assert_eq!(report["filled"][0]["pulse_before"], 98);
    assert_eq!(report["filled"][0]["pulse_after"], 100);

    let second = sync();
    assert_eq!(second.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&second.stderr));
    let report: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(report["sessions"], 1, "idempotent: session count stable");
    assert_eq!(report["filled"].as_array().unwrap().len(), 0, "idempotent: nothing left to fill");

    let conn = rusqlite::Connection::open_with_flags(
        dir.join("t.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM watch_sessions", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 1, "watch_sessions upsert must not duplicate rows");
    let pulse: (i64, i64) = conn.query_row(
        "SELECT pulse_before, pulse_after FROM trainings WHERE id = ?1",
        [training_id], |r| Ok((r.get(0).unwrap(), r.get(1).unwrap()))).unwrap();
    assert_eq!(pulse, (98, 100), "values stored by the first sync must persist");
    let bot_row: (i64, i64) = conn.query_row(
        "SELECT COALESCE(pulse_before, 0), COALESCE(pulse_after, 0) FROM trainings WHERE user_id IS NOT NULL",
        [], |r| Ok((r.get(0).unwrap(), r.get(1).unwrap()))).unwrap();
    assert_eq!(bot_row, (0, 0), "another user's training must keep empty pulse");
}

#[test]
fn write_commands_do_not_initialize_existing_file_without_create() {
    // r3 finding: an existing empty or foreign SQLite file must not be silently
    // turned into a majowuji database — initialization is only via --create
    let dir = temp_dir();

    std::fs::write(dir.join("empty.db"), b"").unwrap();
    let out = run(&dir, &["--db", "empty.db", "log", "jab", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("--create to initialize"));
    assert_eq!(
        std::fs::read(dir.join("empty.db")).unwrap(),
        Vec::<u8>::new(),
        "empty file must stay empty"
    );

    let conn = rusqlite::Connection::open(dir.join("foreign.db")).unwrap();
    conn.execute_batch("CREATE TABLE unrelated (x INTEGER);").unwrap();
    let before = std::fs::read(dir.join("foreign.db")).unwrap();
    let out = run(&dir, &["--db", "foreign.db", "log", "jab", "--json"]);
    assert_failed_clean(&out);
    assert_eq!(std::fs::read(dir.join("foreign.db")).unwrap(), before);

    // sync without --create on the same foreign file must refuse as well
    let out = command(&dir, &["--db", "foreign.db", "intervals", "sync", "--athlete", "i1", "--json"])
        .env("INTERVALS_API_KEY", "test-key")
        .output().unwrap();
    assert_failed_clean(&out);
}

#[test]
fn create_flag_does_not_migrate_existing_outdated_database() {
    // r6 finding (P1): --create initializes only a missing file; an outdated
    // schema is migrated solely by the explicit `migrate` command
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
    let before = std::fs::read(dir.join("old.db")).unwrap();
    let out = run(&dir, &["--db", "old.db", "log", "cross", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));
    assert_eq!(
        std::fs::read(dir.join("old.db")).unwrap(),
        before,
        "schema and records must stay byte-identical"
    );
    let conn = rusqlite::Connection::open_with_flags(
        dir.join("old.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('users', 'watch_sessions')",
        [], |r| r.get(0)).unwrap();
    assert_eq!(tables, 0, "no majowuji tables may be created");
}

#[test]
fn create_flag_does_not_initialize_foreign_existing_file() {
    let dir = temp_dir();
    let conn = rusqlite::Connection::open(dir.join("foreign.db")).unwrap();
    conn.execute_batch("CREATE TABLE unrelated (x INTEGER);").unwrap();
    drop(conn);
    let before = std::fs::read(dir.join("foreign.db")).unwrap();
    let out = run(&dir, &["--db", "foreign.db", "log", "jab", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a majowuji database"));
    assert_eq!(std::fs::read(dir.join("foreign.db")).unwrap(), before);
}

#[test]
fn create_flag_writes_existing_current_database() {
    // --create on an already-initialized database is a plain write, not an error
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    let out = run(&dir, &["--db", "t.db", "log", "cross", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let list = run(&dir, &["--db", "t.db", "list", "--json"]);
    let v: Vec<serde_json::Value> = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(v.len(), 2);
}

#[test]
fn create_flag_initializes_zero_byte_file() {
    let dir = temp_dir();
    std::fs::write(dir.join("z.db"), b"").unwrap();
    let out = run(&dir, &["--db", "z.db", "log", "jab", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let list = run(&dir, &["--db", "z.db", "list", "--json"]);
    let v: Vec<serde_json::Value> = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(v.len(), 1);
}

#[test]
fn create_flag_rejects_existing_nonempty_file_without_tables() {
    // r7 finding (P1): "empty" means 0 bytes, not "no user tables" — a VIEW-only
    // file and a file whose only table was dropped are foreign databases
    let dir = temp_dir();
    let conn = rusqlite::Connection::open(dir.join("view.db")).unwrap();
    conn.execute_batch("CREATE VIEW unrelated AS SELECT 1;").unwrap();
    drop(conn);
    let before = std::fs::read(dir.join("view.db")).unwrap();
    let out = run(&dir, &["--db", "view.db", "log", "jab", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_failed_clean(&out);
    assert_eq!(std::fs::read(dir.join("view.db")).unwrap(), before);

    let conn = rusqlite::Connection::open(dir.join("dropped.db")).unwrap();
    conn.execute_batch("CREATE TABLE tmp (x INTEGER); DROP TABLE tmp;").unwrap();
    drop(conn);
    let before = std::fs::read(dir.join("dropped.db")).unwrap();
    let out = run(&dir, &["--db", "dropped.db", "log", "jab", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_failed_clean(&out);
    assert_eq!(std::fs::read(dir.join("dropped.db")).unwrap(), before);

    // r8: a file with a header but no objects (PRAGMA user_version=42) is
    // non-empty on disk too — size, not the object count, is the criterion
    let conn = rusqlite::Connection::open(dir.join("versioned.db")).unwrap();
    conn.pragma_update(None, "user_version", 42).unwrap();
    drop(conn);
    let before = std::fs::read(dir.join("versioned.db")).unwrap();
    let out = run(&dir, &["--db", "versioned.db", "log", "jab", "-s", "1", "-r", "10", "--create", "--json"]);
    assert_failed_clean(&out);
    assert_eq!(std::fs::read(dir.join("versioned.db")).unwrap(), before);
}

#[test]
fn outdated_trainings_columns_require_migrate() {
    // r8 finding (P1): schema_is_current must check every column the code
    // reads, not only the recently added ones — a trainings table without
    // `notes` passes a narrow check and lets sync write before failing
    let dir = temp_dir();
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER UNIQUE NOT NULL,
                username TEXT,
                first_name TEXT,
                created_at TEXT NOT NULL,
                is_owner BOOLEAN DEFAULT FALSE);
             CREATE TABLE trainings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                exercise TEXT NOT NULL,
                sets INTEGER NOT NULL,
                reps INTEGER NOT NULL,
                duration_secs INTEGER,
                pulse_before INTEGER,
                pulse_after INTEGER,
                user_id INTEGER REFERENCES users(id));
             CREATE TABLE watch_sessions (
                id TEXT PRIMARY KEY,
                start TEXT NOT NULL,
                activity_type TEXT,
                name TEXT,
                elapsed_secs INTEGER NOT NULL,
                avg_hr INTEGER,
                max_hr INTEGER,
                calories INTEGER,
                source TEXT,
                hr_json TEXT NOT NULL);
             INSERT INTO trainings (date, exercise, sets, reps) VALUES ('2026-01-05 12:00:00', 'jab', 3, 50);",
        ).unwrap();
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));
    let out = run(&dir, &["--db", "t.db", "stats", "--json"]);
    assert_failed_clean(&out);
}

#[test]
fn missing_users_table_requires_migrate() {
    // r7 finding (P2): schema_is_current must see a dropped users table,
    // otherwise read commands and --create accept a broken schema as current
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch("DROP TABLE users;").unwrap();
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));

    let out = run(&dir, &["--db", "t.db", "migrate"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn migrate_repairs_schema_missing_notes_column() {
    // r9 finding (P2): a trainings table without `notes` is outdated and the
    // diagnostics point at migrate — migrate must actually add the column,
    // otherwise the advice is a dead end and no CLI path serves the database
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE trainings_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                exercise TEXT NOT NULL,
                sets INTEGER NOT NULL,
                reps INTEGER NOT NULL,
                duration_secs INTEGER,
                pulse_before INTEGER,
                pulse_after INTEGER,
                user_id INTEGER REFERENCES users(id));
             INSERT INTO trainings_new (date, exercise, sets, reps)
                SELECT date, exercise, sets, reps FROM trainings;
             DROP TABLE trainings;
             ALTER TABLE trainings_new RENAME TO trainings;",
        )
        .unwrap();
    }
    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("majowuji migrate"));

    let out = run(&dir, &["--db", "t.db", "migrate"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let out = run(&dir, &["--db", "t.db", "list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn activities_diary_is_newest_first_and_limited() {
    // item 7: the diary lists watch_sessions (not trainings), newest first,
    // with the summary fields the watch reported; -l N limits the output
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO watch_sessions (id, start, activity_type, name, elapsed_secs, avg_hr, max_hr, calories, source, hr_json) VALUES
                ('i1', '2026-10-01T18:00:00+00:00', 'WeightTraining', NULL, 600, NULL, NULL, NULL, 'ZEPP', '[]'),
                ('i3', '2026-10-03T18:00:00+00:00', 'WeightTraining', NULL, 3600, 98, 119, 411, 'ZEPP', '[]'),
                ('i2', '2026-10-02T18:00:00+00:00', NULL, 'Evening round', 1800, 90, 110, 200, 'ZEPP', '[]');",
        )
        .unwrap();
    }

    let out = run(&dir, &["--db", "t.db", "activities", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty(), "stdout is the product: diagnostics to stderr");
    let diary: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = diary.iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["i3", "i2", "i1"]);
    let newest = &diary[0];
    assert_eq!(newest["start"], "2026-10-03T18:00:00Z");
    assert_eq!(newest["activity_type"], "WeightTraining");
    assert_eq!(newest["name"], serde_json::Value::Null);
    assert_eq!(newest["elapsed_secs"], 3600);
    assert_eq!(newest["avg_hr"], 98);
    assert_eq!(newest["max_hr"], 119);
    assert_eq!(newest["calories"], 411);
    assert_eq!(newest["source"], "ZEPP");
    // name-only entry: no activity type, both fields still map independently
    let named = &diary[1];
    assert_eq!(named["activity_type"], serde_json::Value::Null);
    assert_eq!(named["name"], "Evening round");
    // fields the watch did not report stay null
    let oldest = &diary[2];
    assert_eq!(oldest["avg_hr"], serde_json::Value::Null);
    assert_eq!(oldest["max_hr"], serde_json::Value::Null);
    assert_eq!(oldest["calories"], serde_json::Value::Null);
    // the HR stream is not part of the diary
    assert!(newest.get("hr").is_none() && newest.get("hr_json").is_none());

    let out = run(&dir, &["--db", "t.db", "activities", "-l", "2", "--json"]);
    let diary: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = diary.iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["i3", "i2"]);

    // human form: same diary with the ids for cross-reference
    let out = run(&dir, &["--db", "t.db", "activities"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Watch activities:"));
    assert!(text.contains("i3") && text.contains("Evening round"));
}

#[test]
fn activities_empty_diary_is_empty_array() {
    // an initialized database without imported workouts is not an error
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    let out = run(&dir, &["--db", "t.db", "activities", "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let diary: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(diary.is_empty());
}

#[test]
fn activities_requires_existing_database() {
    // read command: a missing database is an error, never a silent empty diary
    let dir = temp_dir();
    let out = run(&dir, &["--db", "nope.db", "activities", "--json"]);
    assert_failed_clean(&out);
    assert!(String::from_utf8_lossy(&out.stderr).contains("database not found"));
}

#[test]
fn activities_does_not_modify_database() {
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO watch_sessions (id, start, activity_type, name, elapsed_secs, avg_hr, max_hr, calories, source, hr_json) VALUES
                ('i1', '2026-10-01T18:00:00+00:00', 'WeightTraining', NULL, 600, 95, 115, 150, 'ZEPP', '[]');",
        )
        .unwrap();
    }
    let before = std::fs::read(dir.join("t.db")).unwrap();
    let out = run(&dir, &["--db", "t.db", "activities", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(std::fs::read(dir.join("t.db")).unwrap(), before);
}

#[test]
fn activities_limit_beyond_i64_shows_all() {
    // a limit larger than i64::MAX must mean "at most N" (= everything here),
    // not wrap into SQLite's negative-LIMIT "no limit" through a different path
    let dir = temp_dir();
    log(&dir, "t.db", "jab");
    {
        let conn = rusqlite::Connection::open(dir.join("t.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO watch_sessions (id, start, activity_type, name, elapsed_secs, avg_hr, max_hr, calories, source, hr_json) VALUES
                ('i1', '2026-10-01T18:00:00+00:00', 'WeightTraining', NULL, 600, 95, 115, 150, 'ZEPP', '[]'),
                ('i2', '2026-10-02T18:00:00+00:00', 'WeightTraining', NULL, 600, 95, 115, 150, 'ZEPP', '[]');",
        )
        .unwrap();
    }
    let huge = usize::MAX.to_string();
    let out = run(&dir, &["--db", "t.db", "activities", "-l", &huge, "--json"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let diary: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(diary.len(), 2);
}
