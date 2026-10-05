//! Intervals.icu import: watch workouts (Zepp -> Intervals.icu) and heart rate of logged sets

use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::db::Training;

pub const API_BASE: &str = "https://intervals.icu/api/v1";

/// A set logged up to this long after the watch workout stopped still belongs to it
pub const GRACE_SECS: i64 = 300;

/// The watch reports 0 until the sensor locks on, so the first reading arrives late;
/// within this window it may still serve as "pulse before" an early logged set
const WARMUP_GRACE_SECS: i64 = 60;

/// Workout recorded by the watch
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchSession {
    pub id: String,
    pub start: DateTime<Utc>,
    pub activity_type: Option<String>,
    pub name: Option<String>,
    pub elapsed_secs: i64,
    pub avg_hr: Option<i32>,
    pub max_hr: Option<i32>,
    pub calories: Option<i32>,
    pub source: Option<String>,
    /// Heart rate samples: (seconds from start, bpm)
    pub hr: Vec<(i64, i32)>,
}

impl WatchSession {
    pub fn end(&self) -> DateTime<Utc> {
        self.start + chrono::Duration::seconds(self.elapsed_secs)
    }

    /// Last sample at or before `offset`; the first sample is used only inside the
    /// warmup window, so a far-future reading is never reported as "pulse before"
    fn hr_at(&self, offset: i64) -> Option<i32> {
        self.hr
            .iter()
            .take_while(|(t, _)| *t <= offset)
            .last()
            .or_else(|| self.hr.first().filter(|(t, _)| *t - offset <= WARMUP_GRACE_SECS))
            .map(|(_, bpm)| *bpm)
    }

    /// Peak in [from, to]
    fn hr_max(&self, from: i64, to: i64) -> Option<i32> {
        self.hr
            .iter()
            .filter(|(t, _)| *t >= from && *t <= to)
            .map(|(_, bpm)| *bpm)
            .max()
    }
}

/// Heart rate assigned to a logged set; a field is None when the workout has
/// no measurable reading for it (e.g. the sensor locked on after the warmup
/// window) — the other field is still filled independently
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PulseLink {
    pub training_id: i64,
    pub session_id: String,
    pub pulse_before: Option<i32>,
    pub pulse_after: Option<i32>,
}

/// Match logged sets to watch workouts by time.
///
/// A set belongs to the closest workout whose window covers it: the workout
/// containing it beats one whose `GRACE_SECS` tail merely reaches it, and a tie
/// (two workouts overlapping the set) goes to the later start. The choice is
/// made for every set before any pulse is computed, so the API reply order can
/// never bind another workout's readings to it. A workout with no HR readings
/// still claims its sets — the set then gets no pulse at all rather than the
// grace-tail readings of an earlier workout. Sets are taken in time order: a
/// set starts where the previous one in the same workout was logged (or at the
/// workout start). `pulse_before` is the heart rate at that point, `pulse_after`
/// the peak up to the moment the set was logged; each is computed independently,
/// so an unavailable one (sensor locked on late) never discards the measured
/// other. Real readings already stored
/// are kept (the database fills each missing field separately); every set still
/// marks where the next starts.
pub fn link_pulses(trainings: &[Training], sessions: &[WatchSession]) -> Vec<PulseLink> {
    use std::collections::HashMap;

    let mut assigned: HashMap<&str, Vec<&Training>> = HashMap::new();
    for t in trainings {
        if t.id.is_none() {
            continue;
        }
        // every session competes, HR-less ones included: a set inside a
        // session without readings must stay unfilled, not inherit the
        // grace-tail pulse of another workout
        let mut best: Option<(i64, DateTime<Utc>, &WatchSession)> = None;
        for s in sessions.iter() {
            let window_end = s.end() + chrono::Duration::seconds(GRACE_SECS);
            if t.date < s.start || t.date > window_end {
                continue;
            }
            let end = s.end();
            // milliseconds, not seconds: a session that ended half a second
            // before the set must not truncate into a tie with the session
            // containing it — the tie would hand the set to the later start
            let dist = if t.date <= end { 0 } else { (t.date - end).num_milliseconds().max(1) };
            let better = match best {
                None => true,
                Some((best_dist, best_start, _)) => {
                    dist < best_dist || (dist == best_dist && s.start > best_start)
                }
            };
            if better {
                best = Some((dist, s.start, s));
            }
        }
        if let Some((_, _, s)) = best {
            assigned.entry(s.id.as_str()).or_default().push(t);
        }
    }

    let mut links = Vec::new();
    for session in sessions.iter().filter(|s| !s.hr.is_empty()) {
        let Some(sets) = assigned.get_mut(session.id.as_str()) else {
            continue;
        };
        sets.sort_by_key(|t| t.date);

        let last_sample = session.hr.last().map(|(t, _)| *t).unwrap_or(0);
        let mut set_start = 0;
        for set in sets {
            let logged_at = (set.date - session.start).num_seconds().min(last_sample);
            let unfilled = !has_pulse(set.pulse_before) || !has_pulse(set.pulse_after);
            if unfilled {
                let before = session.hr_at(set_start);
                let after = session.hr_max(set_start, logged_at);
                if before.is_some() || after.is_some() {
                    links.push(PulseLink {
                        training_id: set.id.unwrap(),
                        session_id: session.id.clone(),
                        pulse_before: before,
                        pulse_after: after,
                    });
                }
            }
            set_start = logged_at;
        }
    }
    links
}

/// 0 bpm is "no reading", not a value (see `zip_hr`)
fn has_pulse(v: Option<i32>) -> bool {
    v.is_some_and(|bpm| bpm > 0)
}

#[derive(Deserialize)]
struct ApiActivity {
    id: String,
    start_date: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    activity_type: Option<String>,
    name: Option<String>,
    elapsed_time: Option<i64>,
    average_heartrate: Option<f64>,
    max_heartrate: Option<f64>,
    calories: Option<f64>,
    source: Option<String>,
}

#[derive(Deserialize)]
struct ApiStream {
    #[serde(rename = "type")]
    stream_type: String,
    data: Vec<Option<f64>>,
}

/// Providers whose uploads are watch recordings. Anything else in the activities
/// window (manual entries, imports from other trackers — they can carry heart
/// rate) must not become a watch session: it would link a wrong pulse to a logged
/// set, and filled fields are never overwritten by the next sync. Exact
/// case-insensitive equality: a prefix match would also admit independent
/// providers like "ZEPPELIN". Value observed in the owner's data: "ZEPP"
/// (production DB, 01-02.10.2026).
const WATCH_SOURCES: &[&str] = &["zepp"];

fn is_watch_source(source: &Option<String>) -> bool {
    source
        .as_deref()
        .is_some_and(|s| WATCH_SOURCES.iter().any(|w| s.eq_ignore_ascii_case(w)))
}

/// Intervals.icu REST client (API key auth)
pub struct Client {
    http: reqwest::Client,
    base: String,
    api_key: String,
    athlete: String,
}

impl Client {
    pub fn new(api_key: String, athlete: String) -> Result<Self> {
        if api_key.trim().is_empty() {
            bail!("Intervals.icu API key is empty");
        }
        if athlete.trim().is_empty() {
            bail!("Intervals.icu athlete id is empty");
        }
        // Override for integration tests and diagnostics: point the client at a
        // mock server instead of the production API. Disabled in release builds:
        // a stray .env value must not be able to redirect the real API key.
        #[cfg(debug_assertions)]
        let base = std::env::var("MAJOWUJI_INTERVALS_API_BASE")
            .unwrap_or_else(|_| API_BASE.to_string());
        #[cfg(not(debug_assertions))]
        let base = API_BASE.to_string();
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self { http, base, api_key, athlete })
    }

    /// Test-only: the same client pointed at a mock server
    #[cfg(test)]
    fn with_base(base: String, api_key: String, athlete: String) -> Result<Self> {
        let mut client = Self::new(api_key, athlete)?;
        client.base = base;
        Ok(client)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(format!("{}{}", self.base, path))
            .basic_auth("API_KEY", Some(&self.api_key))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            bail!("Intervals.icu returned {} for {}", status, path);
        }
        Ok(resp.json().await?)
    }

    /// Watch workouts started between the two dates (inclusive), with heart rate streams
    pub async fn sessions(&self, oldest: NaiveDate, newest: NaiveDate) -> Result<Vec<WatchSession>> {
        let activities: Vec<ApiActivity> = self
            .get(&format!(
                "/athlete/{}/activities?oldest={}&newest={}",
                self.athlete, oldest, newest
            ))
            .await?;

        let mut sessions = Vec::new();
        for a in activities {
            if !is_watch_source(&a.source) {
                continue;
            }
            let Some(start) = a.start_date else { continue };
            let streams: Vec<ApiStream> = self
                .get(&format!("/activity/{}/streams?types=time,heartrate", a.id))
                .await?;
            let hr = zip_hr(&streams);
            sessions.push(WatchSession {
                id: a.id,
                start,
                activity_type: a.activity_type,
                name: a.name,
                elapsed_secs: effective_elapsed(a.elapsed_time, &hr),
                avg_hr: a.average_heartrate.map(|v| v.round() as i32),
                max_hr: a.max_heartrate.map(|v| v.round() as i32),
                calories: a.calories.map(|v| v.round() as i32),
                source: a.source,
                hr,
            });
        }
        Ok(sessions)
    }
}

/// The end of a workout is its reported elapsed time; a missing one falls back to
/// the last heart-rate sample (0 when nothing is known — such sessions carry no
/// heart rate and are skipped by the linker anyway)
fn effective_elapsed(api_elapsed: Option<i64>, hr: &[(i64, i32)]) -> i64 {
    match api_elapsed {
        Some(secs) => secs,
        None => hr.last().map(|(t, _)| *t).unwrap_or(0),
    }
}

/// Pair time and heartrate streams, dropping gaps. The watch reports 0 until the
/// sensor locks on (observed 01.10: first 19 s of a workout), so 0 is a gap too.
fn zip_hr(streams: &[ApiStream]) -> Vec<(i64, i32)> {
    let find = |name: &str| streams.iter().find(|s| s.stream_type == name).map(|s| &s.data);
    let (Some(time), Some(hr)) = (find("time"), find("heartrate")) else {
        return Vec::new();
    };
    time.iter()
        .zip(hr.iter())
        .filter_map(|(t, bpm)| Some((t.as_ref()?.round() as i64, bpm.as_ref()?.round() as i32)))
        .filter(|(_, bpm)| *bpm > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn session(start: DateTime<Utc>, hr: &[(i64, i32)]) -> WatchSession {
        WatchSession {
            id: "i1".into(),
            start,
            activity_type: Some("WeightTraining".into()),
            name: None,
            elapsed_secs: hr.last().map(|(t, _)| *t).unwrap_or(0),
            avg_hr: None,
            max_hr: None,
            calories: None,
            source: Some("ZEPP".into()),
            hr: hr.to_vec(),
        }
    }

    fn set(id: i64, date: DateTime<Utc>) -> Training {
        Training {
            id: Some(id),
            date,
            exercise: "отжимания".into(),
            sets: 1,
            reps: 6,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 30, 22, 53, 29).unwrap()
    }

    fn secs(s: i64) -> chrono::Duration {
        chrono::Duration::seconds(s)
    }

    #[test]
    fn set_logged_after_watch_stopped_gets_session_pulse() {
        // real case 01.10: 67 s workout, set logged 33 s after the watch stopped
        let s = session(t0(), &[(0, 76), (30, 84), (60, 93), (67, 92)]);
        let links = link_pulses(&[set(14, t0() + secs(100))], &[s]);
        assert_eq!(
            links,
            vec![PulseLink {
                training_id: 14,
                session_id: "i1".into(),
                pulse_before: Some(76),
                pulse_after: Some(93)
            }]
        );
    }

    #[test]
    fn consecutive_sets_split_the_stream() {
        let s = session(t0(), &[(0, 70), (50, 120), (100, 90), (150, 130), (200, 95)]);
        let links = link_pulses(&[set(2, t0() + secs(160)), set(1, t0() + secs(60))], &[s]);
        assert_eq!(links.len(), 2);
        assert_eq!((links[0].training_id, links[0].pulse_before, links[0].pulse_after), (1, Some(70), Some(120)));
        assert_eq!((links[1].training_id, links[1].pulse_before, links[1].pulse_after), (2, Some(120), Some(130)));
    }

    #[test]
    fn sets_outside_the_window_are_not_linked() {
        let s = session(t0(), &[(0, 70), (60, 100)]);
        let before = set(1, t0() - secs(1));
        let after = set(2, t0() + secs(60 + GRACE_SECS + 1));
        assert!(link_pulses(&[before, after], &[s]).is_empty());
    }

    #[test]
    fn filled_set_is_kept_but_moves_the_boundary() {
        let s = session(t0(), &[(0, 70), (50, 120), (100, 90), (150, 130)]);
        let mut first = set(1, t0() + secs(60));
        first.pulse_before = Some(65);
        first.pulse_after = Some(118);
        let links = link_pulses(&[first, set(2, t0() + secs(150))], &[s]);
        assert_eq!(links.len(), 1);
        assert_eq!((links[0].training_id, links[0].pulse_before, links[0].pulse_after), (2, Some(120), Some(130)));
    }

    #[test]
    fn session_without_heart_rate_links_nothing() {
        let s = session(t0(), &[]);
        assert!(link_pulses(&[set(1, t0() + secs(10))], &[s]).is_empty());
    }

    #[test]
    fn warmup_first_sample_serves_as_before() {
        // set 1 is logged at 10 s, before the sensor locks on (19 s): its peak is
        // not measurable yet, but the warmup fallback gives the "before" and it is
        // filled alone (r9) — it still moves the boundary, so set 2 gets its
        // "before" from the warmup fallback, the first sample 9 s into the future
        let s = session(t0(), &[(19, 98), (32, 100), (50, 120)]);
        let links = link_pulses(&[set(1, t0() + secs(10)), set(2, t0() + secs(50))], &[s]);
        assert_eq!(links.len(), 2);
        assert_eq!(
            (links[0].training_id, links[0].pulse_before, links[0].pulse_after),
            (1, Some(98), None)
        );
        assert_eq!(
            (links[1].training_id, links[1].pulse_before, links[1].pulse_after),
            (2, Some(98), Some(120))
        );
    }

    #[test]
    fn far_future_first_sample_is_not_used_as_before() {
        // first reading is 120 s after the set: too far to pass as "pulse before"
        let s = session(t0(), &[(180, 110), (200, 120)]);
        assert!(link_pulses(&[set(1, t0() + secs(60))], &[s]).is_empty());
    }

    #[test]
    fn effective_elapsed_falls_back_to_hr_stream() {
        assert_eq!(effective_elapsed(None, &[(0, 70), (600, 90)]), 600);
        // the API value is authoritative even when the stream runs longer
        assert_eq!(effective_elapsed(Some(300), &[(0, 70), (600, 90)]), 300);
        assert_eq!(effective_elapsed(Some(900), &[(0, 70)]), 900);
        assert_eq!(effective_elapsed(None, &[]), 0);
    }

    fn http_json(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    /// One canned HTTP response per incoming connection, in order; returns the
    /// received requests (method, path and headers) for contract assertions
    fn spawn_mock(
        responses: Vec<String>,
    ) -> (String, std::thread::JoinHandle<()>, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen_requests = Arc::clone(&requests);
        let handle = std::thread::spawn(move || {
            // bounded wait: a regression or a bad fixture that makes the client
            // skip an expected request must turn the test red on its assertions,
            // not hang the harness forever in accept()
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            listener.set_nonblocking(true).unwrap();
            for resp in responses {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(pair) => break pair,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() >= deadline {
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(e) => panic!("mock accept failed: {e}"),
                    }
                };
                // accepted sockets inherit the listener's nonblocking mode
                stream.set_nonblocking(false).unwrap();
                let mut seen = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = stream.read(&mut buf).unwrap();
                    seen.extend_from_slice(&buf[..n]);
                    if n == 0 || seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                seen_requests
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&seen).into_owned());
                stream.write_all(resp.as_bytes()).unwrap();
            }
        });
        (format!("http://{}", addr), handle, requests)
    }

    #[tokio::test]
    async fn client_sync_end_to_end_with_mock_api_and_database() {
        let activities = r#"[{
            "id": "a1", "start_date": "2026-10-01T18:00:00Z", "type": "WeightTraining",
            "elapsed_time": null, "average_heartrate": 98.4, "max_heartrate": 119.0,
            "calories": 11.0, "source": "ZEPP" }]"#;
        let streams = r#"[
            {"type": "time", "data": [0, 19, 40, 70]},
            {"type": "heartrate", "data": [0, 98, 100, 119]}]"#;
        let (base, server, requests) = spawn_mock(vec![http_json(activities), http_json(streams)]);

        let client = Client::with_base(base, "key".into(), "i1".into()).unwrap();
        let oldest = chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let newest = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let sessions = client.sessions(oldest, newest).await.unwrap();
        server.join().unwrap();

        // the API contract itself: exact endpoints, date parameters and auth
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2, "one request per endpoint");
        assert!(requests[0].contains("GET /athlete/i1/activities?oldest=2026-09-30&newest=2026-10-02 "));
        assert!(requests[0].to_lowercase().contains("authorization: basic"));
        assert!(requests[1].contains("GET /activity/a1/streams?types=time,heartrate "));

        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.id, "a1");
        assert_eq!(s.start, Utc.with_ymd_and_hms(2026, 10, 1, 18, 0, 0).unwrap());
        // elapsed_time missing in the API response: taken from the last HR sample
        assert_eq!(s.elapsed_secs, 70);
        assert_eq!(s.avg_hr, Some(98));
        assert_eq!(s.max_hr, Some(119));
        // the 0 bpm warmup sample is dropped
        assert_eq!(s.hr, vec![(19, 98), (40, 100), (70, 119)]);

        let dir = std::env::temp_dir().join(format!("majowuji-mock-{}", std::process::id()));
        // a previous run may have crashed before cleanup and a reused PID
        // would inherit its it.db (a stale set breaks the link count below)
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Database::open(dir.join("it.db").to_str().unwrap()).unwrap();
        let training = Training {
            id: None,
            date: Utc.with_ymd_and_hms(2026, 10, 1, 18, 0, 40).unwrap(),
            exercise: "отжимания".into(),
            sets: 1,
            reps: 6,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        };
        let id = db.add_training_cli(&training).unwrap();

        let links = link_pulses(&db.get_trainings().unwrap(), &sessions);
        assert_eq!(links.len(), 1);
        assert_eq!(
            (links[0].training_id, links[0].session_id.as_str(), links[0].pulse_before, links[0].pulse_after),
            (id, "a1", Some(98), Some(100))
        );
        assert_eq!(
            db.fill_training_pulse(id, Some(98), Some(100)).unwrap(),
            Some((Some(98), Some(100)))
        );
        let stored = db.get_trainings().unwrap().into_iter().find(|t| t.id == Some(id)).unwrap();
        assert_eq!((stored.pulse_before, stored.pulse_after), (Some(98), Some(100)));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn link_pulses_assigns_overlapping_set_to_the_containing_session() {
        // Session A ends at t=60 and its grace window still reaches a set logged
        // at t=150; session B spans t=120..420. The set must go to B even though
        // A is returned first: the old per-session loop bound A's readings and
        // the first fill made them permanent (r7 finding, P1)
        let base = Utc.with_ymd_and_hms(2026, 10, 1, 18, 0, 0).unwrap();
        let session_a = WatchSession {
            id: "a".into(),
            start: base,
            activity_type: None,
            name: None,
            elapsed_secs: 60,
            avg_hr: None,
            max_hr: None,
            calories: None,
            source: Some("ZEPP".into()),
            hr: vec![(10, 90), (50, 95)],
        };
        let session_b = WatchSession {
            id: "b".into(),
            start: base + chrono::Duration::seconds(120),
            activity_type: None,
            name: None,
            elapsed_secs: 300,
            avg_hr: None,
            max_hr: None,
            calories: None,
            source: Some("ZEPP".into()),
            hr: vec![(10, 88), (30, 120)],
        };
        let training = Training {
            id: Some(1),
            date: base + chrono::Duration::seconds(150),
            exercise: "jab".into(),
            sets: 1,
            reps: 6,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        };
        let links = link_pulses(&[training], &[session_a, session_b]);
        assert_eq!(links.len(), 1);
        assert_eq!(
            (links[0].training_id, links[0].session_id.as_str(), links[0].pulse_before, links[0].pulse_after),
            (1, "b", Some(88), Some(120))
        );
    }

    #[tokio::test]
    async fn sessions_skip_non_watch_sources() {
        // A manual/other-tracker activity with heart rate must not become a watch
        // session: it would link a wrong pulse and block the real one (filled
        // fields are never overwritten)
        let activities = r#"[{
            "id": "a1", "start_date": "2026-10-01T18:00:00Z", "type": "WeightTraining",
            "elapsed_time": 600, "average_heartrate": 110.0, "max_heartrate": 130.0,
            "calories": 50.0, "source": "STRAVA" }]"#;
        let (base, server, requests) = spawn_mock(vec![http_json(activities)]);
        let client = Client::with_base(base, "key".into(), "i1".into()).unwrap();
        let oldest = chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let newest = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let sessions = client.sessions(oldest, newest).await.unwrap();
        server.join().unwrap();
        assert!(sessions.is_empty(), "non-watch source must be filtered out");
        // filtered before the streams request: exactly one HTTP call happened
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sessions_skip_close_prefix_sources() {
        // prefix lookalike must be rejected too (r2 finding): "ZEPPELIN" is not Zepp
        let activities = r#"[{
            "id": "a1", "start_date": "2026-10-01T18:00:00Z", "type": "WeightTraining",
            "elapsed_time": 600, "average_heartrate": 110.0, "max_heartrate": 130.0,
            "calories": 50.0, "source": "ZEPPELIN" }]"#;
        let (base, server, requests) = spawn_mock(vec![http_json(activities)]);
        let client = Client::with_base(base, "key".into(), "i1".into()).unwrap();
        let oldest = chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let newest = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let sessions = client.sessions(oldest, newest).await.unwrap();
        server.join().unwrap();
        assert!(sessions.is_empty(), "prefix lookalike source must be filtered out");
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn link_pulses_prefers_hr_less_session_over_foreign_grace_tail() {
        // Session B contains the set but has no HR readings; session A's grace
        // tail still reaches it. The set must stay unfilled rather than get
        // A's readings — the old pre-filter made A win because B was invisible
        // to the closest-session choice (r8 finding, P1)
        let base = Utc.with_ymd_and_hms(2026, 10, 1, 18, 0, 0).unwrap();
        let session_a = WatchSession {
            id: "a".into(),
            start: base,
            activity_type: None,
            name: None,
            elapsed_secs: 60,
            avg_hr: None,
            max_hr: None,
            calories: None,
            source: Some("ZEPP".into()),
            hr: vec![(10, 90), (50, 95)],
        };
        let session_b = WatchSession {
            id: "b".into(),
            start: base + chrono::Duration::seconds(120),
            activity_type: None,
            name: None,
            elapsed_secs: 300,
            avg_hr: None,
            max_hr: None,
            calories: None,
            source: Some("ZEPP".into()),
            hr: vec![],
        };
        let training = Training {
            id: Some(1),
            date: base + chrono::Duration::seconds(150),
            exercise: "jab".into(),
            sets: 1,
            reps: 6,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        };
        let links = link_pulses(&[training], &[session_a, session_b]);
        assert!(links.is_empty(), "a set inside an HR-less session gets no pulse");
    }

    #[test]
    fn zero_pulse_counts_as_missing() {
        let s = session(t0(), &[(19, 98), (32, 100)]);
        let mut stale = set(15, t0() + secs(50));
        stale.pulse_before = Some(0);
        stale.pulse_after = Some(100);
        let links = link_pulses(&[stale], &[s]);
        assert_eq!(links.len(), 1);
        assert_eq!((links[0].pulse_before, links[0].pulse_after), (Some(98), Some(100)));
    }

    #[test]
    fn link_pulses_fills_after_when_before_unavailable() {
        // r9 finding (P2): the first sample at 120 s is outside the 60 s warmup
        // window, so "pulse before" at the workout start is unknown — the measured
        // peak up to the set must still be reported, not dropped with the before
        let s = session(t0(), &[(120, 98)]);
        let links = link_pulses(&[set(1, t0() + secs(130))], &[s]);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].pulse_before, None);
        assert_eq!(links[0].pulse_after, Some(98));
    }

    #[test]
    fn link_pulses_prefers_containing_session_over_subsecond_grace_tail() {
        // r9 finding (P2): B ends half a second before the logged set —
        // truncating the gap to whole seconds ties it with the containing A,
        // and the tie would hand the set to the later start B
        let mut a = session(t0(), &[(740, 88)]);
        a.id = "a".into();
        a.elapsed_secs = 800;
        let mut b = session(t0() + secs(700), &[(10, 150)]);
        b.id = "b".into();
        b.elapsed_secs = 50; // ends at t0+750; the set is logged at t0+750.5
        let mut t = set(1, t0() + secs(750) + chrono::Duration::milliseconds(500));
        t.pulse_before = None;
        t.pulse_after = None;
        let links = link_pulses(&[t], &[a, b]);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].session_id, "a");
        assert_eq!(links[0].pulse_after, Some(88));
    }

    #[test]
    fn zip_hr_drops_sensor_warmup_zeros() {
        let streams = vec![
            ApiStream { stream_type: "time".into(), data: vec![Some(0.0), Some(1.0), Some(19.0)] },
            ApiStream { stream_type: "heartrate".into(), data: vec![Some(0.0), Some(0.0), Some(98.0)] },
        ];
        assert_eq!(zip_hr(&streams), vec![(19, 98)]);
    }

    #[test]
    fn zip_hr_drops_gaps() {
        let streams = vec![
            ApiStream { stream_type: "time".into(), data: vec![Some(0.0), Some(1.0), Some(2.0)] },
            ApiStream { stream_type: "heartrate".into(), data: vec![Some(76.0), None, Some(78.0)] },
        ];
        assert_eq!(zip_hr(&streams), vec![(0, 76), (2, 78)]);
    }
}
