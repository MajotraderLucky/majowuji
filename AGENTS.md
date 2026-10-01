# AGENTS.md — majowuji for external agents

Contract for any agent (claude, codex, opencode, goose, hermes) that reads or
writes training data through the `majowuji` CLI. Spawn-per-task: no daemon, no
network, one SQLite file.

## Build and test

```bash
systemd-run --user --scope --unit=rust-tests --slice=rust-build.slice \
  cargo test -j 2 --test cli
systemd-run --user --scope --unit=rust-build --slice=rust-build.slice \
  cargo build -j 2 --release --bin majowuji
# binary: target/release/majowuji
```

## Selecting the database

Always pass the database explicitly — the default `majowuji.db` is relative to
the current directory.

```bash
majowuji --db /path/to/majowuji.db ...
MAJOWUJI_DB=/path/to/majowuji.db majowuji ...
```

Read commands (`list`, `stats`, `tui`) open the database read-only: they never
create, initialize or migrate it. A missing file, an empty file, a non-SQLite file
or a database without the `trainings` table is an error (exit 1), not an empty
result. A database with an outdated schema is also an error: run `majowuji migrate`
once (it updates the schema of an existing non-empty file and adds no records).
No command creates the file silently: `log` and `intervals sync` create and
initialize it only with an explicit `--create`. `--db` must be a file path: empty,
`:memory:` and `file:` URIs are rejected.

## Channels and exit codes

- stdout — the product only (JSON with `--json`). Safe to pipe.
- stderr — diagnostics and errors (`Error: ...`).
- exit `0` — success.
- exit `1` — runtime error: bad database, empty exercise name, SQLite failure.
- exit `2` — invalid arguments (unknown flag, `-s`/`-r` below 1), reported by clap.
- On exit 1 and 2 stdout is empty.

## Commands

| Command                                      | Class | `--json` output                                  |
|----------------------------------------------|-------|--------------------------------------------------|
| `list [-l N] --json`                         | read  | array of trainings, newest first (default N=10)  |
| `stats --json`                               | read  | `{total_trainings, weekly_frequency}`            |
| `stats <exercise> --json`                    | read  | `{exercise, total_volume, suggested_next}`       |
| `log <exercise> -s S -r R [--duration D --pulse-before B --pulse-after A] [-n NOTE] [--create] --json` | write | the stored training with its `id` |

Training object:

```json
{
  "id": 14,
  "date": "2026-09-30T21:55:30.362202162Z",
  "exercise": "jab",
  "sets": 3,
  "reps": 50,
  "duration_secs": null,
  "pulse_before": null,
  "pulse_after": null,
  "notes": "Focus on hip rotation",
  "user_id": null
}
```

`-s` and `-r` must be at least 1. `--duration` is seconds (>= 1) for timed
exercises; `--pulse-before`/`--pulse-after` are bpm in 30..=220. `--create`
initializes a missing database file (schema only, no records). `date` is
RFC 3339 in UTC. `stats <exercise>` matches by case-insensitive substring.
`suggested_next` is `{sets, reps}` or `null` when there is no history for the
exercise.

## Permissions

- `list`, `stats` — free.
- `tui` — interactive terminal UI for a human, not for agents. Running `majowuji`
  without a subcommand also starts it: agents must always pass a subcommand.
- `migrate` — schema update of an existing database; ask the owner first.
- `log` — allowed **without confirmation** (owner decision 2026-10-01). It only
  appends a record and never changes or deletes existing ones.
- `intervals sync` — allowed **without confirmation** (owner decision 2026-10-01).
  It imports watch workouts from Intervals.icu into `watch_sessions` (idempotent)
  and fills only empty `pulse_before`/`pulse_after` of logged sets. Pass the key
  through the environment, never as an argument:
  `INTERVALS_API_KEY="$(pass show majowuji/intervals/api-key | sed -n 1p)" majowuji --db ... intervals sync --athlete <id> --json`
  Output: `{sessions, filled: [{training_id, session_id, pulse_before, pulse_after}]}`.
- Forbidden for agents:
  - editing or deleting rows (no such command; do not use `sqlite3` directly);
  - `bot` — production runs on the owner's server as a systemd unit;
  - reading or printing `.env` (contains the Telegram token);
  - `ansible/` deploy.

## Known limitation

`log` stores `user_id = NULL`. The Telegram bot shows trainings per user and
assigns NULL records to the owner only once, at the owner's first registration.
Records logged through the CLI after that are visible in `list`/`stats`/`tui`
but not in the bot.
