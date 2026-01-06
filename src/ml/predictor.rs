//! Progress prediction using linear regression (linfa)

use chrono::{DateTime, Utc};
use linfa::prelude::*;
use linfa_linear::LinearRegression;
use ndarray::{Array1, Array2};

use crate::db::Training;

/// Minimum data points required for training
const MIN_DATA_POINTS: usize = 3;

/// Progress predictor using linear regression
pub struct ProgressPredictor {
    slope: f64,
    intercept: f64,
    r2_score: f64,
    exercise: String,
    data_points: usize,
    first_date: DateTime<Utc>,
}

/// Prediction result for display
#[derive(Debug, Clone)]
pub struct Prediction {
    pub daily_progress: f64,
    pub week_prediction: f64,
    pub month_prediction: f64,
    pub r2_score: f64,
    pub data_points: usize,
}

impl ProgressPredictor {
    /// Train a predictor from training history for a specific exercise
    pub fn train(trainings: &[Training], exercise: &str) -> Option<Self> {
        // Filter trainings for this exercise
        let exercise_trainings: Vec<_> = trainings
            .iter()
            .filter(|t| t.exercise == exercise)
            .collect();

        if exercise_trainings.len() < MIN_DATA_POINTS {
            return None;
        }

        // Find first training date for this exercise
        let first_date = exercise_trainings
            .iter()
            .map(|t| t.date)
            .min()?;

        // Prepare data: X = days since first training, Y = reps
        let mut x_data: Vec<f64> = Vec::new();
        let mut y_data: Vec<f64> = Vec::new();

        for training in &exercise_trainings {
            let days_offset = (training.date - first_date).num_days() as f64;
            x_data.push(days_offset);
            y_data.push(training.reps as f64);
        }

        let n_samples = x_data.len();

        // Create ndarray structures
        let records = Array2::from_shape_vec(
            (n_samples, 1),
            x_data,
        ).ok()?;

        let targets = Array1::from_vec(y_data);

        // Create dataset
        let dataset = Dataset::new(records.clone(), targets.clone());

        // Train linear regression model
        let model = LinearRegression::default()
            .fit(&dataset)
            .ok()?;

        // Get model parameters
        let params = model.params();
        let slope = params[0];
        let intercept = model.intercept();

        // Calculate R2 score
        let predictions = model.predict(&dataset);
        let r2_score = predictions.r2(&dataset).unwrap_or(0.0);

        Some(Self {
            slope,
            intercept,
            r2_score,
            exercise: exercise.to_string(),
            data_points: n_samples,
            first_date,
        })
    }

    /// Predict reps for a given number of days ahead from now
    pub fn predict_reps(&self, days_ahead: i32) -> f64 {
        let now = Utc::now();
        let days_from_start = (now - self.first_date).num_days() as f64;
        let future_day = days_from_start + days_ahead as f64;
        self.slope * future_day + self.intercept
    }

    /// Get current predicted level (reps today)
    pub fn current_level(&self) -> f64 {
        self.predict_reps(0)
    }

    /// Get daily progress (slope)
    pub fn daily_progress(&self) -> f64 {
        self.slope
    }

    /// Get R2 score (model fit quality, 0-1)
    pub fn r2_score(&self) -> f64 {
        self.r2_score
    }

    /// Get number of data points used for training
    pub fn data_points(&self) -> usize {
        self.data_points
    }

    /// Get full prediction for display
    pub fn get_prediction(&self) -> Prediction {
        Prediction {
            daily_progress: self.slope,
            week_prediction: self.predict_reps(7),
            month_prediction: self.predict_reps(30),
            r2_score: self.r2_score,
            data_points: self.data_points,
        }
    }

    /// Format prediction for bot message
    pub fn format_prediction(&self) -> String {
        let pred = self.get_prediction();

        // Format daily progress with sign
        let trend_str = if pred.daily_progress >= 0.0 {
            format!("+{:.1}", pred.daily_progress)
        } else {
            format!("{:.1}", pred.daily_progress)
        };

        format!(
            "--- ML Прогноз ---\n\
            Тренд: {} повт./день\n\
            Через неделю: ~{:.0} повторений\n\
            Через месяц: ~{:.0} повторений",
            trend_str,
            pred.week_prediction.max(0.0),
            pred.month_prediction.max(0.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_training(exercise: &str, reps: i32, days_ago: i64) -> Training {
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
    fn test_predictor_insufficient_data() {
        // Only 2 data points - should return None
        let trainings = vec![
            create_training("pushups", 10, 7),
            create_training("pushups", 12, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups");
        assert!(predictor.is_none());
    }

    #[test]
    fn test_predictor_no_matching_exercise() {
        let trainings = vec![
            create_training("squats", 10, 14),
            create_training("squats", 12, 7),
            create_training("squats", 14, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups");
        assert!(predictor.is_none());
    }

    #[test]
    fn test_predictor_linear_trend() {
        // Create perfect linear progression: 10, 12, 14 over 14 days
        let trainings = vec![
            create_training("pushups", 10, 14),
            create_training("pushups", 12, 7),
            create_training("pushups", 14, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();

        // Daily progress should be approximately 4/14 ≈ 0.286
        let daily = predictor.daily_progress();
        assert!(daily > 0.2 && daily < 0.4, "Daily progress: {}", daily);

        // R2 should be very high for perfect linear data
        assert!(predictor.r2_score() > 0.9, "R2 score: {}", predictor.r2_score());
    }

    #[test]
    fn test_predict_future_reps() {
        let trainings = vec![
            create_training("pushups", 10, 14),
            create_training("pushups", 12, 7),
            create_training("pushups", 14, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();

        // Current level should be close to 14
        let current = predictor.current_level();
        assert!(current > 13.0 && current < 15.0, "Current level: {}", current);

        // Week ahead should be higher
        let week = predictor.predict_reps(7);
        assert!(week > current, "Week prediction {} should be > current {}", week, current);
    }

    #[test]
    fn test_data_points_count() {
        let trainings = vec![
            create_training("pushups", 10, 21),
            create_training("pushups", 11, 14),
            create_training("pushups", 12, 7),
            create_training("pushups", 13, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();
        assert_eq!(predictor.data_points(), 4);
    }

    #[test]
    fn test_get_prediction() {
        let trainings = vec![
            create_training("pushups", 10, 14),
            create_training("pushups", 12, 7),
            create_training("pushups", 14, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();
        let pred = predictor.get_prediction();

        assert!(pred.daily_progress > 0.0);
        assert!(pred.week_prediction > 0.0);
        assert!(pred.month_prediction > pred.week_prediction);
        assert_eq!(pred.data_points, 3);
    }

    #[test]
    fn test_format_prediction() {
        let trainings = vec![
            create_training("pushups", 10, 14),
            create_training("pushups", 12, 7),
            create_training("pushups", 14, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();
        let formatted = predictor.format_prediction();

        assert!(formatted.contains("ML Прогноз"));
        assert!(formatted.contains("Тренд:"));
        assert!(formatted.contains("неделю"));
        assert!(formatted.contains("месяц"));
    }

    #[test]
    fn test_negative_trend() {
        // Decreasing performance
        let trainings = vec![
            create_training("pushups", 20, 14),
            create_training("pushups", 18, 7),
            create_training("pushups", 16, 0),
        ];
        let predictor = ProgressPredictor::train(&trainings, "pushups").unwrap();

        // Daily progress should be negative
        assert!(predictor.daily_progress() < 0.0);

        // Week prediction should be lower than current
        assert!(predictor.predict_reps(7) < predictor.current_level());
    }
}
