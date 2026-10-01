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

    /// Last sample at or before `offset`, or the first sample
    fn hr_at(&self, offset: i64) -> Option<i32> {
        self.hr
            .iter()
            .take_while(|(t, _)| *t <= offset)
            .last()
            .or(self.hr.first())
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

/// Heart rate assigned to a logged set
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PulseLink {
    pub training_id: i64,
    pub session_id: String,
    pub pulse_before: i32,
    pub pulse_after: i32,
}

/// Match logged sets to watch workouts by time.
///
/// A set belongs to a workout when it was logged between the workout start and
/// its end plus `GRACE_SECS`. Sets are taken in time order: a set starts where the
/// previous one in the same workout was logged (or at the workout start).
/// `pulse_before` is the heart rate at that point, `pulse_after` the peak up to the
/// moment the set was logged. Real readings already stored are kept (the database
/// fills each missing field separately); every set still marks where the next starts.
pub fn link_pulses(trainings: &[Training], sessions: &[WatchSession]) -> Vec<PulseLink> {
    let mut links = Vec::new();
    for session in sessions.iter().filter(|s| !s.hr.is_empty()) {
        let window_end = session.end() + chrono::Duration::seconds(GRACE_SECS);
        let mut sets: Vec<&Training> = trainings
            .iter()
            .filter(|t| t.id.is_some() && t.date >= session.start && t.date <= window_end)
            .collect();
        sets.sort_by_key(|t| t.date);

        let last_sample = session.hr.last().map(|(t, _)| *t).unwrap_or(0);
        let mut set_start = 0;
        for set in sets {
            let logged_at = (set.date - session.start).num_seconds().min(last_sample);
            let unfilled = !has_pulse(set.pulse_before) || !has_pulse(set.pulse_after);
            if unfilled {
                if let (Some(before), Some(after)) =
                    (session.hr_at(set_start), session.hr_max(set_start, logged_at))
                {
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

/// Intervals.icu REST client (API key auth)
pub struct Client {
    http: reqwest::Client,
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
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self { http, api_key, athlete })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(format!("{}{}", API_BASE, path))
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
            let Some(start) = a.start_date else { continue };
            let streams: Vec<ApiStream> = self
                .get(&format!("/activity/{}/streams?types=time,heartrate", a.id))
                .await?;
            sessions.push(WatchSession {
                id: a.id,
                start,
                activity_type: a.activity_type,
                name: a.name,
                elapsed_secs: a.elapsed_time.unwrap_or(0),
                avg_hr: a.average_heartrate.map(|v| v.round() as i32),
                max_hr: a.max_heartrate.map(|v| v.round() as i32),
                calories: a.calories.map(|v| v.round() as i32),
                source: a.source,
                hr: zip_hr(&streams),
            });
        }
        Ok(sessions)
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
            vec![PulseLink { training_id: 14, session_id: "i1".into(), pulse_before: 76, pulse_after: 93 }]
        );
    }

    #[test]
    fn consecutive_sets_split_the_stream() {
        let s = session(t0(), &[(0, 70), (50, 120), (100, 90), (150, 130), (200, 95)]);
        let links = link_pulses(&[set(2, t0() + secs(160)), set(1, t0() + secs(60))], &[s]);
        assert_eq!(links.len(), 2);
        assert_eq!((links[0].training_id, links[0].pulse_before, links[0].pulse_after), (1, 70, 120));
        assert_eq!((links[1].training_id, links[1].pulse_before, links[1].pulse_after), (2, 120, 130));
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
        assert_eq!((links[0].training_id, links[0].pulse_before, links[0].pulse_after), (2, 120, 130));
    }

    #[test]
    fn session_without_heart_rate_links_nothing() {
        let s = session(t0(), &[]);
        assert!(link_pulses(&[set(1, t0() + secs(10))], &[s]).is_empty());
    }

    #[test]
    fn zero_pulse_counts_as_missing() {
        let s = session(t0(), &[(19, 98), (32, 100)]);
        let mut stale = set(15, t0() + secs(50));
        stale.pulse_before = Some(0);
        stale.pulse_after = Some(100);
        let links = link_pulses(&[stale], &[s]);
        assert_eq!(links.len(), 1);
        assert_eq!((links[0].pulse_before, links[0].pulse_after), (98, 100));
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
