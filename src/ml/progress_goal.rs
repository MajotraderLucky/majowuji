//! Fatigue-aware progress goals
//!
//! Shows realistic goals BEFORE exercise, accounting for accumulated
//! fatigue from prior exercises in the session.

use std::collections::HashMap;
use chrono::{DateTime, FixedOffset, Utc};

use crate::db::Training;
use crate::exercises::{find_exercise_by_name, MuscleGroup};

/// Moscow timezone offset (UTC+3)
fn moscow_tz() -> FixedOffset {
    FixedOffset::east_opt(3 * 3600).unwrap()
}

/// Session context representing fatigue state
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    /// Load per muscle group done TODAY before target exercise
    pub prior_load: HashMap<MuscleGroup, i32>,
    /// Total session duration in seconds
    pub session_duration_secs: i32,
    /// Number of exercises done today
    pub exercises_done: usize,
}

/// Historical session data point
#[derive(Debug, Clone)]
pub struct HistoricalSession {
    pub date: DateTime<Utc>,
    pub context_before: SessionContext,
    pub exercise_name: String,
    pub achieved_reps: i32,
}

/// Confidence level for goal prediction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GoalConfidence {
    /// < 3 similar sessions
    Low,
    /// 3-5 similar sessions
    Medium,
    /// > 5 similar sessions
    High,
}

impl GoalConfidence {
    pub fn label(&self) -> &'static str {
        match self {
            GoalConfidence::Low => "(мало данных)",
            GoalConfidence::Medium => "",
            GoalConfidence::High => "",
        }
    }
}

/// Progress goal with fatigue adjustment
#[derive(Debug, Clone)]
pub struct ProgressGoal {
    /// Target reps (adjusted for fatigue + 1 for progress)
    pub target_reps: i32,
    /// Confidence level
    pub confidence: GoalConfidence,
    /// Fatigue factor (0.0 = fresh, 1.0 = fatigued)
    pub fatigue_factor: f32,
    /// Number of similar sessions found
    pub similar_sessions: usize,
    /// Sets done today for this exercise
    pub today_sets: usize,
    /// Total reps done today for this exercise
    pub today_reps: i32,
    /// Fatigued muscle groups (for display)
    pub fatigued_muscles: Vec<MuscleGroup>,
}

impl ProgressGoal {
    /// Format goal for bot message
    pub fn format(&self) -> String {
        let confidence_label = self.confidence.label();

        let fatigue_info = if self.fatigue_factor > 0.1 {
            let muscles: Vec<&str> = self.fatigued_muscles
                .iter()
                .take(2)
                .map(|m| m.name_ru())
                .collect();
            if muscles.is_empty() {
                String::new()
            } else {
                format!("\n  Усталость: {:.0}% ({})",
                    self.fatigue_factor * 100.0,
                    muscles.join(", "))
            }
        } else {
            String::new()
        };

        format!(
            "Сегодня: {} подх., {} повт.\n  Цель: ~{} повторов {}{}",
            self.today_sets,
            self.today_reps,
            self.target_reps,
            confidence_label,
            fatigue_info
        )
    }

    /// Format short goal for recommendation
    pub fn format_short(&self) -> String {
        let confidence_label = self.confidence.label();
        format!(
            "Сегодня: {} подх. | Цель: ~{} {}",
            self.today_sets,
            self.target_reps,
            confidence_label
        )
    }
}

/// Goal calculator with session context matching
pub struct GoalCalculator;

impl GoalCalculator {
    /// Minimum similarity threshold for matching sessions
    const MIN_SIMILARITY: f32 = 0.5;

    /// Fatigue sensitivity: 50 reps = ~63% fatigue contribution
    const FATIGUE_K: f32 = 50.0;

    /// Calculate fatigue-aware goal for an exercise
    pub fn calculate(
        trainings: &[Training],
        exercise_name: &str,
    ) -> Option<ProgressGoal> {
        let exercise = find_exercise_by_name(exercise_name)?;

        // Build current session context
        let current_context = Self::build_current_context(trainings);

        // Calculate fatigue factor
        let fatigue_factor = Self::fatigue_factor(&current_context, exercise.muscle_groups);

        // Find fatigued muscles
        let fatigued_muscles: Vec<MuscleGroup> = exercise.muscle_groups
            .iter()
            .filter(|m| current_context.prior_load.get(*m).copied().unwrap_or(0) > 0)
            .copied()
            .collect();

        // Get today's stats for this exercise
        let today = Utc::now().with_timezone(&moscow_tz()).date_naive();
        let today_exercises: Vec<_> = trainings
            .iter()
            .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() == today)
            .filter(|t| t.exercise == exercise_name)
            .collect();
        let today_sets = today_exercises.len();
        let today_reps: i32 = today_exercises.iter().map(|t| t.reps).sum();

        // Find similar historical sessions
        let similar = Self::find_similar_sessions(trainings, exercise_name, &current_context);

        // Calculate target reps
        let target_reps = if similar.is_empty() {
            // No similar sessions - use personal best or default
            let best = trainings
                .iter()
                .filter(|t| t.exercise == exercise_name)
                .map(|t| t.reps)
                .max()
                .unwrap_or(10);
            // Adjust for fatigue
            ((best as f32) * (1.0 - fatigue_factor * 0.3)).round() as i32
        } else {
            // Weighted average of similar sessions + 1 for progress
            let weighted_sum: f32 = similar.iter()
                .map(|(s, sim)| s.achieved_reps as f32 * sim)
                .sum();
            let weight_total: f32 = similar.iter().map(|(_, sim)| sim).sum();
            let avg = weighted_sum / weight_total;
            (avg + 1.0).round() as i32
        };

        let confidence = match similar.len() {
            0..=2 => GoalConfidence::Low,
            3..=5 => GoalConfidence::Medium,
            _ => GoalConfidence::High,
        };

        Some(ProgressGoal {
            target_reps: target_reps.max(1),
            confidence,
            fatigue_factor,
            similar_sessions: similar.len(),
            today_sets,
            today_reps,
            fatigued_muscles,
        })
    }

    /// Build session context from today's trainings
    fn build_current_context(trainings: &[Training]) -> SessionContext {
        let today = Utc::now().with_timezone(&moscow_tz()).date_naive();

        let today_trainings: Vec<_> = trainings
            .iter()
            .filter(|t| t.date.with_timezone(&moscow_tz()).date_naive() == today)
            .collect();

        if today_trainings.is_empty() {
            return SessionContext::default();
        }

        // Accumulate load per muscle group
        let mut prior_load: HashMap<MuscleGroup, i32> = HashMap::new();
        let mut total_duration = 0;

        for t in &today_trainings {
            if let Some(ex) = find_exercise_by_name(&t.exercise) {
                for muscle in ex.muscle_groups {
                    *prior_load.entry(*muscle).or_insert(0) += t.reps;
                }
            }
            total_duration += t.duration_secs.unwrap_or(0);
        }

        SessionContext {
            prior_load,
            session_duration_secs: total_duration,
            exercises_done: today_trainings.len(),
        }
    }

    /// Calculate fatigue factor for target muscle groups
    fn fatigue_factor(context: &SessionContext, muscles: &[MuscleGroup]) -> f32 {
        if muscles.is_empty() {
            return 0.0;
        }

        let mut total = 0.0;
        for muscle in muscles {
            let load = context.prior_load.get(muscle).copied().unwrap_or(0);
            // Exponential saturation: fatigue = 1 - e^(-load/k)
            let fatigue = 1.0 - (-load as f32 / Self::FATIGUE_K).exp();
            total += fatigue;
        }

        total / muscles.len() as f32
    }

    /// Find historical sessions with similar context
    fn find_similar_sessions(
        trainings: &[Training],
        exercise_name: &str,
        current_context: &SessionContext,
    ) -> Vec<(HistoricalSession, f32)> {
        // Group trainings by day
        let sessions_by_day = Self::group_by_day(trainings);

        let mut similar = Vec::new();
        let today = Utc::now().with_timezone(&moscow_tz()).date_naive();

        for (date, day_trainings) in sessions_by_day {
            // Skip today
            if date == today {
                continue;
            }

            // Sort by timestamp
            let mut sorted = day_trainings.clone();
            sorted.sort_by_key(|t| t.date);

            // Reconstruct context before each exercise
            let mut accumulated_load: HashMap<MuscleGroup, i32> = HashMap::new();
            let mut session_duration = 0;
            let mut exercises_done = 0;

            for training in sorted {
                // Build context BEFORE this exercise
                let context_before = SessionContext {
                    prior_load: accumulated_load.clone(),
                    session_duration_secs: session_duration,
                    exercises_done,
                };

                // If this is our target exercise, compute similarity
                if training.exercise == exercise_name {
                    let similarity = Self::compute_similarity(&context_before, current_context);

                    if similarity >= Self::MIN_SIMILARITY {
                        similar.push((
                            HistoricalSession {
                                date: training.date,
                                context_before,
                                exercise_name: training.exercise.clone(),
                                achieved_reps: training.reps,
                            },
                            similarity,
                        ));
                    }
                }

                // Update accumulated load
                if let Some(ex) = find_exercise_by_name(&training.exercise) {
                    for muscle in ex.muscle_groups {
                        *accumulated_load.entry(*muscle).or_insert(0) += training.reps;
                    }
                }
                session_duration += training.duration_secs.unwrap_or(0);
                exercises_done += 1;
            }
        }

        similar
    }

    /// Group trainings by day
    fn group_by_day(trainings: &[Training]) -> HashMap<chrono::NaiveDate, Vec<&Training>> {
        let mut by_day: HashMap<chrono::NaiveDate, Vec<&Training>> = HashMap::new();

        for t in trainings {
            let date = t.date.with_timezone(&moscow_tz()).date_naive();
            by_day.entry(date).or_default().push(t);
        }

        by_day
    }

    /// Compute similarity between two session contexts
    fn compute_similarity(historical: &SessionContext, current: &SessionContext) -> f32 {
        // Collect all muscle groups from both contexts
        let mut all_muscles: Vec<&MuscleGroup> = historical.prior_load.keys().collect();
        for m in current.prior_load.keys() {
            if !all_muscles.contains(&m) {
                all_muscles.push(m);
            }
        }

        if all_muscles.is_empty() {
            // Both contexts are empty (fresh start) = perfect match
            return 1.0;
        }

        let mut total_diff = 0.0;

        for muscle in &all_muscles {
            let hist_load = historical.prior_load.get(*muscle).copied().unwrap_or(0);
            let curr_load = current.prior_load.get(*muscle).copied().unwrap_or(0);

            let max_load = hist_load.max(curr_load).max(1) as f32;
            let diff = (hist_load - curr_load).abs() as f32 / max_load;
            total_diff += diff;
        }

        let avg_diff = total_diff / all_muscles.len() as f32;

        // Convert difference to similarity
        1.0 - avg_diff.min(1.0)
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
            duration_secs: Some(60),
            pulse_before: None,
            pulse_after: None,
            notes: None,
            user_id: None,
        }
    }

    #[test]
    fn test_empty_context_no_fatigue() {
        let context = SessionContext::default();
        let muscles = &[MuscleGroup::Chest, MuscleGroup::Triceps];
        let fatigue = GoalCalculator::fatigue_factor(&context, muscles);
        assert_eq!(fatigue, 0.0);
    }

    #[test]
    fn test_fatigue_after_pushups() {
        let mut context = SessionContext::default();
        context.prior_load.insert(MuscleGroup::Chest, 50);
        context.prior_load.insert(MuscleGroup::Triceps, 50);

        let muscles = &[MuscleGroup::Chest, MuscleGroup::Triceps];
        let fatigue = GoalCalculator::fatigue_factor(&context, muscles);

        // 50 reps with k=50 should give ~63% fatigue per muscle
        assert!(fatigue > 0.5 && fatigue < 0.7, "Fatigue: {}", fatigue);
    }

    #[test]
    fn test_fatigue_partial_overlap() {
        let mut context = SessionContext::default();
        context.prior_load.insert(MuscleGroup::Chest, 30);
        // Triceps not loaded

        let muscles = &[MuscleGroup::Chest, MuscleGroup::Triceps];
        let fatigue = GoalCalculator::fatigue_factor(&context, muscles);

        // Only half the muscles are fatigued
        assert!(fatigue > 0.2 && fatigue < 0.4, "Fatigue: {}", fatigue);
    }

    #[test]
    fn test_similarity_same_context() {
        let context = SessionContext::default();
        let similarity = GoalCalculator::compute_similarity(&context, &context);
        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_similarity_different_context() {
        let mut hist = SessionContext::default();
        hist.prior_load.insert(MuscleGroup::Chest, 50);

        let curr = SessionContext::default();

        let similarity = GoalCalculator::compute_similarity(&hist, &curr);
        // Completely different context
        assert!(similarity < 0.5, "Similarity: {}", similarity);
    }

    #[test]
    fn test_goal_no_history() {
        let trainings = vec![];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");

        // No data at all - should return None or default goal
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert_eq!(g.today_sets, 0);
        assert_eq!(g.confidence, GoalConfidence::Low);
    }

    #[test]
    fn test_goal_with_history() {
        let trainings = vec![
            create_training("отжимания на кулаках", 10, 7),
            create_training("отжимания на кулаках", 12, 6),
            create_training("отжимания на кулаках", 11, 5),
            create_training("отжимания на кулаках", 13, 4),
        ];

        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());

        let g = goal.unwrap();
        // Target should be around average + 1 = ~12
        assert!(g.target_reps >= 10 && g.target_reps <= 15,
            "Target: {}", g.target_reps);
    }

    #[test]
    fn test_goal_format() {
        let goal = ProgressGoal {
            target_reps: 15,
            confidence: GoalConfidence::Low,
            fatigue_factor: 0.35,
            similar_sessions: 2,
            today_sets: 1,
            today_reps: 12,
            fatigued_muscles: vec![MuscleGroup::Chest, MuscleGroup::Triceps],
        };

        let formatted = goal.format();
        assert!(formatted.contains("Сегодня: 1 подх."));
        assert!(formatted.contains("Цель: ~15"));
        assert!(formatted.contains("мало данных"));
        assert!(formatted.contains("Усталость:"));
    }

    #[test]
    fn test_goal_format_short() {
        let goal = ProgressGoal {
            target_reps: 15,
            confidence: GoalConfidence::Medium,
            fatigue_factor: 0.0,
            similar_sessions: 4,
            today_sets: 2,
            today_reps: 25,
            fatigued_muscles: vec![],
        };

        let formatted = goal.format_short();
        assert!(formatted.contains("Сегодня: 2 подх."));
        assert!(formatted.contains("Цель: ~15"));
        assert!(!formatted.contains("мало данных"));
    }

    #[test]
    fn test_confidence_levels() {
        assert_eq!(GoalConfidence::Low.label(), "(мало данных)");
        assert_eq!(GoalConfidence::Medium.label(), "");
        assert_eq!(GoalConfidence::High.label(), "");
    }
}
