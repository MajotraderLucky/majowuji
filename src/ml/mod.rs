//! ML module - Training predictions and recommendations
//!
//! Features:
//! - Muscle group load tracking
//! - Exercise recommendations based on balance
//! - Progress prediction using linear regression (linfa)

pub mod muscle_tracker;
pub mod recommender;
pub mod predictor;

pub use muscle_tracker::MuscleTracker;
pub use recommender::Recommender;
pub use predictor::ProgressPredictor;

use crate::db::Training;

/// Training analytics
pub struct Analytics {
    trainings: Vec<Training>,
}

impl Analytics {
    pub fn new(trainings: Vec<Training>) -> Self {
        Self { trainings }
    }

    /// Calculate total volume (sets * reps) for an exercise
    pub fn total_volume(&self, exercise: &str) -> i32 {
        self.trainings
            .iter()
            .filter(|t| t.exercise.to_lowercase().contains(&exercise.to_lowercase()))
            .map(|t| t.sets * t.reps)
            .sum()
    }

    /// Get training frequency (sessions per week)
    pub fn weekly_frequency(&self) -> f64 {
        if self.trainings.is_empty() {
            return 0.0;
        }

        let dates: Vec<_> = self.trainings.iter().map(|t| t.date.date_naive()).collect();
        if dates.len() < 2 {
            return 0.0;
        }

        let first = dates.last().unwrap();
        let last = dates.first().unwrap();
        let days = (*last - *first).num_days() as f64;

        if days == 0.0 {
            return self.trainings.len() as f64;
        }

        (self.trainings.len() as f64 / days) * 7.0
    }

    /// Predict next training load (simple moving average)
    pub fn predict_next_load(&self, exercise: &str) -> Option<(i32, i32)> {
        let recent: Vec<_> = self.trainings
            .iter()
            .filter(|t| t.exercise.to_lowercase().contains(&exercise.to_lowercase()))
            .take(5)
            .collect();

        if recent.is_empty() {
            return None;
        }

        let avg_sets = recent.iter().map(|t| t.sets).sum::<i32>() / recent.len() as i32;
        let avg_reps = recent.iter().map(|t| t.reps).sum::<i32>() / recent.len() as i32;

        // Slight progression suggestion
        Some((avg_sets, avg_reps + 1))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_training(exercise: &str, sets: i32, reps: i32) -> Training {
        Training {
            id: None,
            date: Utc::now(),
            exercise: exercise.to_string(),
            sets,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    fn create_training_days_ago(exercise: &str, sets: i32, reps: i32, days_ago: i64) -> Training {
        Training {
            id: None,
            date: Utc::now() - chrono::Duration::days(days_ago),
            exercise: exercise.to_string(),
            sets,
            reps,
            duration_secs: None,
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    #[test]
    fn test_analytics_new() {
        let analytics = Analytics::new(vec![]);
        assert_eq!(analytics.trainings.len(), 0);
    }

    #[test]
    fn test_total_volume_single_exercise() {
        let trainings = vec![
            create_training("отжимания на кулаках", 3, 10), // 3 * 10 = 30
        ];
        let analytics = Analytics::new(trainings);
        assert_eq!(analytics.total_volume("отжимания"), 30);
    }

    #[test]
    fn test_total_volume_multiple_entries() {
        let trainings = vec![
            create_training("отжимания на кулаках", 3, 10), // 30
            create_training("отжимания на кулаках", 2, 15), // 30
        ];
        let analytics = Analytics::new(trainings);
        assert_eq!(analytics.total_volume("отжимания"), 60);
    }

    #[test]
    fn test_total_volume_case_insensitive() {
        let trainings = vec![
            create_training("Отжимания на кулаках", 2, 10),
        ];
        let analytics = Analytics::new(trainings);
        assert_eq!(analytics.total_volume("отжимания"), 20);
    }

    #[test]
    fn test_total_volume_empty() {
        let analytics = Analytics::new(vec![]);
        assert_eq!(analytics.total_volume("отжимания"), 0);
    }

    #[test]
    fn test_total_volume_not_found() {
        let trainings = vec![
            create_training("приседания", 3, 10),
        ];
        let analytics = Analytics::new(trainings);
        assert_eq!(analytics.total_volume("отжимания"), 0);
    }

    #[test]
    fn test_weekly_frequency_empty() {
        let analytics = Analytics::new(vec![]);
        assert_eq!(analytics.weekly_frequency(), 0.0);
    }

    #[test]
    fn test_weekly_frequency_single_training() {
        let trainings = vec![
            create_training("отжимания", 3, 10),
        ];
        let analytics = Analytics::new(trainings);
        assert_eq!(analytics.weekly_frequency(), 0.0);
    }

    #[test]
    fn test_weekly_frequency_same_day() {
        let trainings = vec![
            create_training("отжимания", 3, 10),
            create_training("приседания", 3, 20),
        ];
        let analytics = Analytics::new(trainings);
        // Both on same day - should return count
        assert_eq!(analytics.weekly_frequency(), 2.0);
    }

    #[test]
    fn test_weekly_frequency_over_week() {
        let trainings = vec![
            create_training("отжимания", 3, 10),
            create_training_days_ago("приседания", 3, 20, 7),
        ];
        let analytics = Analytics::new(trainings);
        // 2 trainings over 7 days = 2/7 * 7 = 2 per week
        let freq = analytics.weekly_frequency();
        assert!((freq - 2.0).abs() < 0.1, "Expected ~2, got {}", freq);
    }

    #[test]
    fn test_predict_next_load_empty() {
        let analytics = Analytics::new(vec![]);
        assert!(analytics.predict_next_load("отжимания").is_none());
    }

    #[test]
    fn test_predict_next_load_not_found() {
        let trainings = vec![
            create_training("приседания", 3, 10),
        ];
        let analytics = Analytics::new(trainings);
        assert!(analytics.predict_next_load("отжимания").is_none());
    }

    #[test]
    fn test_predict_next_load_single() {
        let trainings = vec![
            create_training("отжимания", 3, 10),
        ];
        let analytics = Analytics::new(trainings);
        let prediction = analytics.predict_next_load("отжимания").unwrap();
        // Should suggest avg_sets and avg_reps + 1 for progression
        assert_eq!(prediction.0, 3); // sets
        assert_eq!(prediction.1, 11); // reps + 1
    }

    #[test]
    fn test_predict_next_load_multiple() {
        let trainings = vec![
            create_training("отжимания", 3, 10),
            create_training("отжимания", 3, 12),
            create_training("отжимания", 3, 14),
        ];
        let analytics = Analytics::new(trainings);
        let prediction = analytics.predict_next_load("отжимания").unwrap();
        // avg sets = 3, avg reps = 12, prediction = (3, 13)
        assert_eq!(prediction.0, 3);
        assert_eq!(prediction.1, 13);
    }

    #[test]
    fn test_predict_next_load_partial_match() {
        let trainings = vec![
            create_training("отжимания на кулаках", 2, 20),
        ];
        let analytics = Analytics::new(trainings);
        let prediction = analytics.predict_next_load("отжимания");
        assert!(prediction.is_some());
        assert_eq!(prediction.unwrap(), (2, 21));
    }
}
