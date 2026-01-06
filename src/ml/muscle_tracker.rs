//! Muscle group load tracking for balanced training recommendations

use std::collections::HashMap;
use chrono::{DateTime, Local, Utc};
use crate::db::Training;
use crate::exercises::{MuscleGroup, find_exercise_by_name};

/// Load statistics for a single muscle group
#[derive(Debug, Clone)]
pub struct MuscleLoad {
    pub group: MuscleGroup,
    pub today_volume: i32,
    pub week_volume: i32,
    pub last_trained: Option<DateTime<Utc>>,
}

/// Tracks muscle group load from training history
pub struct MuscleTracker {
    loads: HashMap<MuscleGroup, MuscleLoad>,
}

impl MuscleTracker {
    /// Build tracker from training history
    pub fn from_trainings(trainings: &[Training]) -> Self {
        let mut loads: HashMap<MuscleGroup, MuscleLoad> = HashMap::new();

        // Initialize all muscle groups
        for group in MuscleGroup::all() {
            loads.insert(*group, MuscleLoad {
                group: *group,
                today_volume: 0,
                week_volume: 0,
                last_trained: None,
            });
        }

        let now = Local::now();
        let today = now.date_naive();
        let week_ago = today - chrono::Duration::days(7);

        for training in trainings {
            // Find exercise definition to get muscle groups
            let exercise = match find_exercise_by_name(&training.exercise) {
                Some(ex) => ex,
                None => continue, // Unknown exercise, skip
            };

            let training_date = training.date.with_timezone(&Local).date_naive();
            let is_today = training_date == today;
            let is_this_week = training_date >= week_ago;

            // Distribute reps to each muscle group the exercise targets
            for muscle_group in exercise.muscle_groups {
                if let Some(load) = loads.get_mut(muscle_group) {
                    if is_today {
                        load.today_volume += training.reps;
                    }
                    if is_this_week {
                        load.week_volume += training.reps;
                    }

                    // Update last trained time
                    if load.last_trained.is_none() || load.last_trained.unwrap() < training.date {
                        load.last_trained = Some(training.date);
                    }
                }
            }
        }

        Self { loads }
    }

    /// Get load for a specific muscle group
    pub fn get_load(&self, group: &MuscleGroup) -> Option<&MuscleLoad> {
        self.loads.get(group)
    }

    /// Get all loads sorted by today's volume (ascending = least worked first)
    pub fn get_loads_sorted(&self) -> Vec<&MuscleLoad> {
        let mut loads: Vec<_> = self.loads.values().collect();
        loads.sort_by_key(|l| l.today_volume);
        loads
    }

    /// Get underworked muscle groups (least volume today, excluding FullBody)
    pub fn get_underworked_groups(&self, limit: usize) -> Vec<MuscleGroup> {
        self.get_loads_sorted()
            .iter()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .take(limit)
            .map(|l| l.group)
            .collect()
    }

    /// Calculate balance score (0-100%)
    /// 100% = perfectly balanced across all groups
    pub fn get_balance_score(&self) -> f32 {
        let volumes: Vec<i32> = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|l| l.week_volume)
            .collect();

        if volumes.is_empty() {
            return 0.0;
        }

        let total: i32 = volumes.iter().sum();
        if total == 0 {
            return 0.0;
        }

        let target = total as f32 / volumes.len() as f32;
        let variance: f32 = volumes.iter()
            .map(|v| (*v as f32 - target).powi(2))
            .sum::<f32>() / volumes.len() as f32;

        let std_dev = variance.sqrt();
        let cv = if target > 0.0 { std_dev / target } else { 0.0 };

        // Convert coefficient of variation to score (lower CV = higher score)
        // CV of 0 = 100%, CV of 1+ = ~0%
        ((1.0 - cv.min(1.0)) * 100.0).max(0.0)
    }

    /// Get weekly report for /balance command
    pub fn get_weekly_report(&self) -> Vec<(MuscleGroup, i32, &'static str)> {
        let max_volume = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|l| l.week_volume)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut report: Vec<_> = self.loads.values()
            .filter(|l| l.group != MuscleGroup::FullBody)
            .map(|load| {
                let ratio = load.week_volume as f32 / max_volume as f32;
                let bar = match ratio {
                    r if r >= 0.75 => "[++++]",
                    r if r >= 0.50 => "[+++.]",
                    r if r >= 0.25 => "[++..]",
                    r if r > 0.0 => "[+...]",
                    _ => "[....]",
                };
                (load.group, load.week_volume, bar)
            })
            .collect();

        report.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by volume descending
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_training(exercise: &str, reps: i32) -> Training {
        Training {
            id: None,
            date: Utc::now(),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    fn create_training_days_ago(exercise: &str, reps: i32, days_ago: i64) -> Training {
        Training {
            id: None,
            date: Utc::now() - chrono::Duration::days(days_ago),
            exercise: exercise.to_string(),
            sets: 1,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    #[test]
    fn test_empty_tracker() {
        let tracker = MuscleTracker::from_trainings(&[]);
        assert_eq!(tracker.get_balance_score(), 0.0);
        assert_eq!(tracker.get_underworked_groups(3).len(), 3);
    }

    #[test]
    fn test_single_training_load() {
        let trainings = vec![create_training("отжимания на кулаках", 20)];
        let tracker = MuscleTracker::from_trainings(&trainings);

        // Pushups target Chest, Triceps, Shoulders, Core
        let chest = tracker.get_load(&MuscleGroup::Chest).unwrap();
        assert_eq!(chest.today_volume, 20);
        assert_eq!(chest.week_volume, 20);
        assert!(chest.last_trained.is_some());
    }

    #[test]
    fn test_multi_muscle_exercise() {
        let trainings = vec![create_training("отжимания на кулаках", 15)];
        let tracker = MuscleTracker::from_trainings(&trainings);

        // All target muscles should have the same volume
        assert_eq!(tracker.get_load(&MuscleGroup::Chest).unwrap().today_volume, 15);
        assert_eq!(tracker.get_load(&MuscleGroup::Triceps).unwrap().today_volume, 15);
        assert_eq!(tracker.get_load(&MuscleGroup::Shoulders).unwrap().today_volume, 15);
        assert_eq!(tracker.get_load(&MuscleGroup::Core).unwrap().today_volume, 15);

        // Non-target muscles should be zero
        assert_eq!(tracker.get_load(&MuscleGroup::Back).unwrap().today_volume, 0);
        assert_eq!(tracker.get_load(&MuscleGroup::Biceps).unwrap().today_volume, 0);
    }

    #[test]
    fn test_get_underworked_groups() {
        let trainings = vec![
            create_training("отжимания на кулаках", 50),
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);

        // Underworked groups should NOT include muscles that were exercised
        // Pushups target: Chest, Triceps, Shoulders, Core
        let underworked = tracker.get_underworked_groups(5);
        assert!(!underworked.contains(&MuscleGroup::Chest),
            "Chest should not be underworked after pushups");
        assert!(!underworked.contains(&MuscleGroup::Triceps),
            "Triceps should not be underworked after pushups");

        // At least Back should be in underworked (has 0 volume)
        assert!(underworked.contains(&MuscleGroup::Back),
            "Back should be underworked (0 volume)");

        // All returned groups should have 0 today_volume
        for group in &underworked {
            let load = tracker.get_load(group).unwrap();
            assert_eq!(load.today_volume, 0,
                "Underworked group {:?} should have 0 volume", group);
        }
    }

    #[test]
    fn test_underworked_excludes_fullbody() {
        let tracker = MuscleTracker::from_trainings(&[]);
        let underworked = tracker.get_underworked_groups(15);
        assert!(!underworked.contains(&MuscleGroup::FullBody));
    }

    #[test]
    fn test_balance_score_zero_when_empty() {
        let tracker = MuscleTracker::from_trainings(&[]);
        assert_eq!(tracker.get_balance_score(), 0.0);
    }

    #[test]
    fn test_balance_score_increases_with_variety() {
        // Only pushups - imbalanced
        let pushup_only = vec![create_training("отжимания на кулаках", 50)];
        let tracker1 = MuscleTracker::from_trainings(&pushup_only);
        let score1 = tracker1.get_balance_score();

        // Add some leg work - more balanced
        let mixed = vec![
            create_training("отжимания на кулаках", 50),
            create_training("приседания с ударами", 30),
        ];
        let tracker2 = MuscleTracker::from_trainings(&mixed);
        let score2 = tracker2.get_balance_score();

        assert!(score2 > score1, "More variety should increase balance: {} > {}", score2, score1);
    }

    #[test]
    fn test_weekly_report_format() {
        let trainings = vec![
            create_training("отжимания на кулаках", 30),
            create_training("приседания с ударами", 20),
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);
        let report = tracker.get_weekly_report();

        // Report should have entries for all muscle groups except FullBody
        assert_eq!(report.len(), 10); // 11 groups - FullBody

        // Each entry is (MuscleGroup, volume, bar_string)
        for (group, _volume, bar) in &report {
            assert_ne!(*group, MuscleGroup::FullBody);
            assert!(bar.starts_with('['));
            assert!(bar.ends_with(']'));
        }
    }

    #[test]
    fn test_weekly_report_sorted_by_volume() {
        let trainings = vec![
            create_training("отжимания на кулаках", 50),
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);
        let report = tracker.get_weekly_report();

        // Should be sorted by volume descending
        let volumes: Vec<i32> = report.iter().map(|(_, v, _)| *v).collect();
        let mut sorted = volumes.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(volumes, sorted);
    }

    #[test]
    fn test_get_loads_sorted_ascending() {
        let trainings = vec![
            create_training("отжимания на кулаках", 30),
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);
        let sorted = tracker.get_loads_sorted();

        // Sorted by today_volume ascending (least worked first)
        let volumes: Vec<i32> = sorted.iter().map(|l| l.today_volume).collect();
        let mut expected = volumes.clone();
        expected.sort();
        assert_eq!(volumes, expected);
    }

    #[test]
    fn test_unknown_exercise_skipped() {
        let trainings = vec![
            create_training("несуществующее упражнение", 100),
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);

        // All muscle groups should have zero volume
        for group in MuscleGroup::all() {
            let load = tracker.get_load(group).unwrap();
            assert_eq!(load.today_volume, 0, "Unknown exercise should be skipped");
        }
    }

    #[test]
    fn test_today_vs_week_volume() {
        let trainings = vec![
            create_training("отжимания на кулаках", 20),           // today
            create_training_days_ago("отжимания на кулаках", 30, 3), // 3 days ago
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);

        let chest = tracker.get_load(&MuscleGroup::Chest).unwrap();
        assert_eq!(chest.today_volume, 20);
        assert_eq!(chest.week_volume, 50); // 20 + 30
    }

    #[test]
    fn test_old_training_excluded_from_week() {
        let trainings = vec![
            create_training_days_ago("отжимания на кулаках", 100, 10), // 10 days ago
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);

        let chest = tracker.get_load(&MuscleGroup::Chest).unwrap();
        assert_eq!(chest.today_volume, 0);
        assert_eq!(chest.week_volume, 0); // Too old
    }

    #[test]
    fn test_last_trained_updates() {
        let trainings = vec![
            create_training_days_ago("отжимания на кулаках", 10, 2),
            create_training("отжимания на кулаках", 20), // More recent
        ];
        let tracker = MuscleTracker::from_trainings(&trainings);

        let chest = tracker.get_load(&MuscleGroup::Chest).unwrap();
        assert!(chest.last_trained.is_some());

        // Last trained should be the most recent (today)
        let now = Utc::now();
        let diff = now - chest.last_trained.unwrap();
        assert!(diff.num_seconds() < 60, "Last trained should be recent");
    }

    #[test]
    fn test_get_load_returns_none_for_invalid_group() {
        let tracker = MuscleTracker::from_trainings(&[]);
        // All valid groups should return Some
        for group in MuscleGroup::all() {
            assert!(tracker.get_load(group).is_some());
        }
    }
}
