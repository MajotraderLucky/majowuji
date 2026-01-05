//! ML module - Training predictions and recommendations
//!
//! Future features:
//! - Progress prediction based on historical data
//! - Optimal training load recommendations
//! - Recovery time estimation
//! - Technique improvement suggestions

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

// TODO: Add more sophisticated ML models when linfa is enabled
// - Linear regression for progress prediction
// - Clustering for workout pattern analysis
// - Time series forecasting for performance trends
