//! Fatigue-aware progress goals
//!
//! Shows realistic goals BEFORE exercise, accounting for accumulated
//! fatigue from prior exercises in the session.

use std::collections::HashMap;
use chrono::{DateTime, FixedOffset, Utc};

use crate::db::Training;
use crate::exercises::{find_exercise_by_name, MuscleGroup};

/// Days to consolidate a new record before challenging to beat it
const RECORD_CONSOLIDATION_DAYS: i64 = 7;

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
    /// Achieved value (reps or seconds for timed exercises)
    pub achieved_value: i32,
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
    /// Target value (reps or seconds for timed exercises) - fatigue-adjusted
    pub target_value: i32,
    /// Personal best (reps or seconds)
    pub personal_best: Option<i32>,
    /// Simple target: personal best + 1 (not fatigue-adjusted)
    pub beat_record_target: Option<i32>,
    /// Is this a timed exercise?
    pub is_timed: bool,
    /// Confidence level
    pub confidence: GoalConfidence,
    /// Fatigue factor (0.0 = fresh, 1.0 = fatigued)
    pub fatigue_factor: f32,
    /// Number of similar sessions found
    pub similar_sessions: usize,
    /// Sets done today for this exercise
    pub today_sets: usize,
    /// Total value done today (reps or seconds)
    pub today_value: i32,
    /// Fatigued muscle groups (for display)
    pub fatigued_muscles: Vec<MuscleGroup>,
    /// Average over last 7 days
    pub avg_7_days: Option<f32>,
    /// Average over last 14 days
    pub avg_14_days: Option<f32>,
    /// Date when personal best was achieved
    pub record_date: Option<DateTime<Utc>>,
    /// True if in consolidation period (need to confirm record level)
    pub is_consolidating: bool,
    /// Days remaining in current consolidation window
    pub consolidation_days_left: Option<i32>,
    /// True if user reached record level within current 7-day window
    pub record_confirmed: bool,
}

impl ProgressGoal {
    /// Format goal for bot message
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        // Today's stats
        let today_str = if self.is_timed {
            format!("Сегодня: {} подх., {}", self.today_sets, Self::format_duration(self.today_value))
        } else {
            format!("Сегодня: {} подх., {} повт.", self.today_sets, self.today_value)
        };
        lines.push(today_str);

        // Personal best - with consolidation or beat target
        if let Some(best) = self.personal_best {
            if self.is_consolidating {
                // Consolidation period - show record with days remaining
                let days_str = self.consolidation_days_left
                    .map(|d| format!(", {} дн.", d))
                    .unwrap_or_default();
                if self.is_timed {
                    lines.push(format!("  Рекорд: {} (закрепляем{})",
                        Self::format_duration(best), days_str));
                } else {
                    lines.push(format!("  Рекорд: {} (закрепляем{})", best, days_str));
                }
            } else if let Some(beat) = self.beat_record_target {
                // Confirmed - challenge to beat
                if self.is_timed {
                    lines.push(format!("  Рекорд: {} → побей: {}",
                        Self::format_duration(best),
                        Self::format_duration(beat)));
                } else {
                    lines.push(format!("  Рекорд: {} → побей: {}", best, beat));
                }
            }
        }

        // Smart target (show if different from simple +1)
        let dominated_by_beat = self.beat_record_target
            .map(|beat| self.target_value == beat)
            .unwrap_or(false);

        if !dominated_by_beat {
            // Build explanation for the smart target
            let explanation = if self.fatigue_factor > 0.1 {
                let muscles: Vec<&str> = self.fatigued_muscles
                    .iter()
                    .take(2)
                    .map(|m| m.name_ru())
                    .collect();
                if muscles.is_empty() {
                    "с учётом усталости".to_string()
                } else {
                    format!("усталость {}", muscles.join(", "))
                }
            } else if self.similar_sessions >= 3 {
                format!("по {} похожим тренировкам", self.similar_sessions)
            } else {
                "прогноз".to_string()
            };

            if self.is_timed {
                lines.push(format!("  ML: ~{} ({})",
                    Self::format_duration(self.target_value),
                    explanation));
            } else {
                lines.push(format!("  ML: ~{} ({})", self.target_value, explanation));
            }
        }

        lines.join("\n")
    }

    /// Format short goal for recommendation
    pub fn format_short(&self) -> String {
        let mut parts = Vec::new();

        // Today's sets
        parts.push(format!("Сегодня: {} подх.", self.today_sets));

        // Average (if available)
        if let Some(avg) = self.avg_7_days {
            if self.is_timed {
                parts.push(format!("Сред: {}", Self::format_duration(avg as i32)));
            } else {
                parts.push(format!("Сред: {:.0}", avg));
            }
        }

        if let Some(best) = self.personal_best {
            if self.is_consolidating {
                // Consolidation period - show record with days remaining
                let days_str = self.consolidation_days_left
                    .map(|d| format!(", {} дн.", d))
                    .unwrap_or_default();
                if self.is_timed {
                    parts.push(format!("Рекорд: {} (закрепляем{})",
                        Self::format_duration(best), days_str));
                } else {
                    parts.push(format!("Рекорд: {} (закрепляем{})", best, days_str));
                }
            } else if let Some(beat) = self.beat_record_target {
                // Confirmed - challenge to beat
                if self.is_timed {
                    parts.push(format!("Рекорд: {} → побей: {}",
                        Self::format_duration(best),
                        Self::format_duration(beat)));
                } else {
                    parts.push(format!("Рекорд: {} → побей: {}", best, beat));
                }

                // ML target if different
                if self.target_value != beat {
                    let explanation = if self.fatigue_factor > 0.1 {
                        "усталость"
                    } else if self.similar_sessions >= 3 {
                        "ML"
                    } else {
                        "прогноз"
                    };
                    if self.is_timed {
                        parts.push(format!("{}: ~{}", explanation, Self::format_duration(self.target_value)));
                    } else {
                        parts.push(format!("{}: ~{}", explanation, self.target_value));
                    }
                }
            }
        } else {
            // No history - show default target
            let confidence_label = self.confidence.label();
            if self.is_timed {
                parts.push(format!("Цель: ~{} {}", Self::format_duration(self.target_value), confidence_label));
            } else {
                parts.push(format!("Цель: ~{} {}", self.target_value, confidence_label));
            }
        }

        parts.join(" | ")
    }

    /// Format duration in human-readable form
    pub(crate) fn format_duration(secs: i32) -> String {
        if secs >= 60 {
            let mins = secs / 60;
            let remaining = secs % 60;
            if remaining > 0 {
                format!("{}м {}с", mins, remaining)
            } else {
                format!("{}м", mins)
            }
        } else {
            format!("{}с", secs)
        }
    }
}

/// Goal calculator with session context matching
pub struct GoalCalculator;

impl GoalCalculator {
    /// Minimum similarity threshold for matching sessions
    const MIN_SIMILARITY: f32 = 0.5;

    /// Fatigue sensitivity: 50 reps = ~63% fatigue contribution
    const FATIGUE_K: f32 = 50.0;

    /// Find personal best value and the date when it was achieved
    fn find_personal_best_with_date(
        trainings: &[Training],
        exercise_name: &str,
        is_timed: bool,
    ) -> Option<(i32, DateTime<Utc>)> {
        let filtered: Vec<_> = trainings
            .iter()
            .filter(|t| t.exercise == exercise_name)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        // Find the best value
        let best_value = if is_timed {
            filtered.iter().filter_map(|t| t.duration_secs).max()?
        } else {
            filtered.iter().map(|t| t.reps).max()?
        };

        // Find the EARLIEST date when this best was achieved (breakthrough date)
        // For consolidation: we track when the record was first set, not repeated
        let best_date = filtered
            .iter()
            .filter(|t| {
                if is_timed {
                    t.duration_secs == Some(best_value)
                } else {
                    t.reps == best_value
                }
            })
            .map(|t| t.date)
            .min()?;

        Some((best_value, best_date))
    }

    /// Check if user reached personal_best within the last N days
    fn has_confirmation_in_window(
        trainings: &[Training],
        exercise_name: &str,
        personal_best: i32,
        is_timed: bool,
        window_days: i64,
    ) -> bool {
        let cutoff = Utc::now() - chrono::Duration::days(window_days);
        trainings
            .iter()
            .filter(|t| t.exercise == exercise_name && t.date >= cutoff)
            .any(|t| {
                if is_timed {
                    t.duration_secs.unwrap_or(0) >= personal_best
                } else {
                    t.reps >= personal_best
                }
            })
    }

    /// Calculate fatigue-aware goal for an exercise
    pub fn calculate(
        trainings: &[Training],
        exercise_name: &str,
    ) -> Option<ProgressGoal> {
        let exercise = find_exercise_by_name(exercise_name)?;
        let is_timed = exercise.is_timed;

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

        // For timed exercises, sum duration_secs; for rep-based, sum reps
        let today_value: i32 = if is_timed {
            today_exercises.iter().filter_map(|t| t.duration_secs).sum()
        } else {
            today_exercises.iter().map(|t| t.reps).sum()
        };

        // Find personal best with date for this exercise
        let (personal_best, record_date) = Self::find_personal_best_with_date(
            trainings, exercise_name, is_timed
        ).map(|(v, d)| (Some(v), Some(d)))
        .unwrap_or((None, None));

        // Enhanced consolidation logic:
        // - Must confirm (reach) record level within 7-day window to unlock progression
        // - If not confirmed within 7 days, extend consolidation another 7 days
        let now = Utc::now();
        let days_since_record = record_date
            .map(|date| (now - date).num_days())
            .unwrap_or(0);

        // Check if user confirmed the record in the current 7-day window
        let record_confirmed = personal_best
            .map(|pb| Self::has_confirmation_in_window(
                trainings, exercise_name, pb, is_timed, RECORD_CONSOLIDATION_DAYS
            ))
            .unwrap_or(false);

        // Consolidation logic:
        // - First 7 days after record: always consolidating (stabilize the new level)
        // - After 7 days: if confirmed in window → can challenge, else → extend consolidation
        let is_consolidating = if personal_best.is_none() {
            false  // No record yet - no consolidation
        } else if days_since_record < RECORD_CONSOLIDATION_DAYS {
            true  // Within initial 7-day window
        } else {
            !record_confirmed  // After 7 days: consolidate if NOT confirmed in last 7 days
        };

        // Calculate days left in current consolidation window
        let consolidation_days_left = if is_consolidating {
            let days_in_window = days_since_record % RECORD_CONSOLIDATION_DAYS;
            Some((RECORD_CONSOLIDATION_DAYS - days_in_window) as i32)
        } else {
            None
        };

        // Challenge only if NOT consolidating
        let beat_record_target = if is_consolidating {
            None  // Don't challenge during consolidation
        } else {
            personal_best.map(|best| best + 1)
        };

        // Find similar historical sessions for fatigue-adjusted target
        let similar = Self::find_similar_sessions(trainings, exercise_name, &current_context, is_timed);

        // Calculate fatigue-adjusted target value
        let target_value = if similar.is_empty() {
            // No similar sessions - use personal best or default, adjusted for fatigue
            let base = personal_best.unwrap_or(if is_timed { 60 } else { 10 });
            let raw_target = base + 1;
            ((raw_target as f32) * (1.0 - fatigue_factor * 0.3)).round() as i32
        } else {
            // Weighted average of similar sessions + progress increment
            let weighted_sum: f32 = similar.iter()
                .map(|(s, sim)| s.achieved_value as f32 * sim)
                .sum();
            let weight_total: f32 = similar.iter().map(|(_, sim)| sim).sum();
            let avg = weighted_sum / weight_total;
            (avg + 1.0).round() as i32
        };

        // Confidence based on total attempts
        let total_attempts = trainings
            .iter()
            .filter(|t| t.exercise == exercise_name)
            .count();

        let confidence = match total_attempts {
            0 => GoalConfidence::Low,
            1..=2 => GoalConfidence::Low,
            3..=5 => GoalConfidence::Medium,
            _ => GoalConfidence::High,
        };

        // Calculate averages
        let (avg_7_days, avg_14_days) = Self::calculate_averages(trainings, exercise_name, is_timed);

        Some(ProgressGoal {
            target_value: target_value.max(1),
            personal_best,
            beat_record_target,
            is_timed,
            confidence,
            fatigue_factor,
            similar_sessions: similar.len(),
            today_sets,
            today_value,
            fatigued_muscles,
            avg_7_days,
            avg_14_days,
            record_date,
            is_consolidating,
            consolidation_days_left,
            record_confirmed,
        })
    }

    /// Calculate average values for last 7 and 14 days
    fn calculate_averages(trainings: &[Training], exercise_name: &str, is_timed: bool) -> (Option<f32>, Option<f32>) {
        let now = Utc::now();

        let calc_avg = |days: i64| -> Option<f32> {
            let cutoff = now - chrono::Duration::days(days);
            let recent: Vec<_> = trainings
                .iter()
                .filter(|t| t.exercise == exercise_name && t.date >= cutoff)
                .collect();

            if recent.is_empty() {
                None
            } else if is_timed {
                let sum: i32 = recent.iter().filter_map(|t| t.duration_secs).sum();
                Some(sum as f32 / recent.len() as f32)
            } else {
                let sum: i32 = recent.iter().map(|t| t.reps).sum();
                Some(sum as f32 / recent.len() as f32)
            }
        };

        (calc_avg(7), calc_avg(14))
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
        is_timed: bool,
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

            for (exercises_done, training) in sorted.into_iter().enumerate() {
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
                        // Use duration_secs for timed exercises, reps otherwise
                        let achieved_value = if is_timed {
                            training.duration_secs.unwrap_or(0)
                        } else {
                            training.reps
                        };
                        similar.push((
                            HistoricalSession {
                                date: training.date,
                                context_before,
                                exercise_name: training.exercise.clone(),
                                achieved_value,
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
        assert!(g.target_value >= 10 && g.target_value <= 15,
            "Target: {}", g.target_value);
    }

    #[test]
    fn test_goal_format() {
        // Test with fatigue - ML target differs from simple +1
        let goal = ProgressGoal {
            target_value: 12, // Different from beat_record_target due to fatigue
            personal_best: Some(14),
            beat_record_target: Some(15),
            is_timed: false,
            confidence: GoalConfidence::Low,
            fatigue_factor: 0.35,
            similar_sessions: 2,
            today_sets: 1,
            today_value: 10,
            fatigued_muscles: vec![MuscleGroup::Chest, MuscleGroup::Triceps],
            avg_7_days: Some(13.5),
            avg_14_days: Some(12.8),
            record_date: Some(Utc::now() - chrono::Duration::days(10)), // Old record
            is_consolidating: false,
            consolidation_days_left: None,
            record_confirmed: true,
        };

        let formatted = goal.format();
        assert!(formatted.contains("Сегодня: 1 подх."), "Format: {}", formatted);
        assert!(formatted.contains("Рекорд: 14 → побей: 15"), "Format: {}", formatted);
        assert!(formatted.contains("ML: ~12"), "Should show ML target: {}", formatted);
        assert!(formatted.contains("усталость"), "Should mention fatigue: {}", formatted);
    }

    #[test]
    fn test_goal_format_short() {
        let goal = ProgressGoal {
            target_value: 15,
            personal_best: Some(14),
            beat_record_target: Some(15),
            is_timed: false,
            confidence: GoalConfidence::Medium,
            fatigue_factor: 0.0,
            similar_sessions: 4,
            today_sets: 2,
            today_value: 25,
            fatigued_muscles: vec![],
            avg_7_days: Some(14.2),
            avg_14_days: Some(13.8),
            record_date: Some(Utc::now() - chrono::Duration::days(10)), // Old record
            is_consolidating: false,
            consolidation_days_left: None,
            record_confirmed: true,
        };

        let formatted = goal.format_short();
        assert!(formatted.contains("Сегодня: 2 подх."), "Format: {}", formatted);
        assert!(formatted.contains("Сред: 14"), "Should show average: {}", formatted);
        assert!(formatted.contains("Рекорд: 14 → побей: 15"), "Format: {}", formatted);
    }

    #[test]
    fn test_confidence_levels() {
        assert_eq!(GoalConfidence::Low.label(), "(мало данных)");
        assert_eq!(GoalConfidence::Medium.label(), "");
        assert_eq!(GoalConfidence::High.label(), "");
    }

    #[test]
    fn test_timed_exercise_format() {
        let goal = ProgressGoal {
            target_value: 180, // 3 minutes
            personal_best: Some(170), // 2m 50s
            beat_record_target: Some(180), // 3m
            is_timed: true,
            confidence: GoalConfidence::Medium,
            fatigue_factor: 0.0,
            similar_sessions: 4,
            today_sets: 1,
            today_value: 120, // 2 minutes
            fatigued_muscles: vec![],
            avg_7_days: Some(150.0),
            avg_14_days: Some(145.0),
            record_date: Some(Utc::now() - chrono::Duration::days(10)), // Old record
            is_consolidating: false,
            consolidation_days_left: None,
            record_confirmed: true,
        };

        let formatted = goal.format();
        assert!(formatted.contains("2м"), "Should show today's duration in minutes: {}", formatted);
        assert!(formatted.contains("побей: 3м"), "Should show beat target in minutes: {}", formatted);

        let short = goal.format_short();
        assert!(short.contains("3м"), "Short format should show target in minutes: {}", short);
    }

    #[test]
    fn test_format_duration() {
        // Test duration formatting helper
        assert_eq!(ProgressGoal::format_duration(45), "45с");
        assert_eq!(ProgressGoal::format_duration(60), "1м");
        assert_eq!(ProgressGoal::format_duration(90), "1м 30с");
        assert_eq!(ProgressGoal::format_duration(180), "3м");
        assert_eq!(ProgressGoal::format_duration(169), "2м 49с");
    }

    // ===== Consolidation Period Tests =====

    #[test]
    fn test_find_personal_best_with_date_no_history() {
        let trainings: Vec<Training> = vec![];
        let result = GoalCalculator::find_personal_best_with_date(
            &trainings, "отжимания на кулаках", false
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_find_personal_best_with_date_single_record() {
        let trainings = vec![
            create_training("отжимания на кулаках", 15, 3),
        ];
        let result = GoalCalculator::find_personal_best_with_date(
            &trainings, "отжимания на кулаках", false
        );
        assert!(result.is_some());
        let (best, _date) = result.unwrap();
        assert_eq!(best, 15);
    }

    #[test]
    fn test_find_personal_best_with_date_multiple_same_record() {
        // Same value achieved twice - should return EARLIEST date (breakthrough)
        let trainings = vec![
            create_training("отжимания на кулаках", 15, 10), // Old - first breakthrough
            create_training("отжимания на кулаках", 15, 2),  // Recent - confirmation
        ];
        let result = GoalCalculator::find_personal_best_with_date(
            &trainings, "отжимания на кулаках", false
        );
        assert!(result.is_some());
        let (best, date) = result.unwrap();
        assert_eq!(best, 15);
        // Should return the EARLIEST date (breakthrough date)
        let days_ago = (Utc::now() - date).num_days();
        assert!(days_ago >= 9, "Should be earliest date (breakthrough), got {} days ago", days_ago);
    }

    #[test]
    fn test_consolidation_period_recent_record() {
        // Record set 3 days ago - should be in consolidation
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 3),
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(g.is_consolidating, "Record from 3 days ago should be consolidating");
        assert!(g.beat_record_target.is_none(), "No beat target during consolidation");
        assert_eq!(g.personal_best, Some(20));
    }

    #[test]
    fn test_consolidation_period_old_record() {
        // Record set 10 days ago, confirmed within last 7 days → should NOT be consolidating
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 10), // Record breakthrough
            create_training("отжимания на кулаках", 20, 3),  // Confirmation within window
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(!g.is_consolidating, "Should unlock after confirmation in window");
        assert_eq!(g.beat_record_target, Some(21));
        assert!(g.record_confirmed);
    }

    #[test]
    fn test_consolidation_boundary_exactly_7_days() {
        // Record set exactly 7 days ago, confirmed within last 7 days → should NOT be consolidating
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 7), // Record breakthrough (boundary)
            create_training("отжимания на кулаках", 20, 2), // Confirmation within window
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(!g.is_consolidating, "Should unlock after confirmation (7 days + confirmed)");
        assert_eq!(g.beat_record_target, Some(21));
        assert!(g.record_confirmed);
    }

    #[test]
    fn test_consolidation_format_during_period() {
        let goal = ProgressGoal {
            target_value: 20,
            personal_best: Some(20),
            beat_record_target: None,
            is_timed: false,
            confidence: GoalConfidence::High,
            fatigue_factor: 0.0,
            similar_sessions: 5,
            today_sets: 1,
            today_value: 18,
            fatigued_muscles: vec![],
            avg_7_days: Some(19.0),
            avg_14_days: Some(18.0),
            record_date: Some(Utc::now() - chrono::Duration::days(2)),
            is_consolidating: true,
            consolidation_days_left: Some(5),
            record_confirmed: false,
        };

        let formatted = goal.format();
        assert!(formatted.contains("Рекорд: 20 (закрепляем, 5 дн.)"), "Format: {}", formatted);
        assert!(!formatted.contains("побей"), "Should not contain 'побей': {}", formatted);
    }

    #[test]
    fn test_consolidation_format_short_during_period() {
        let goal = ProgressGoal {
            target_value: 20,
            personal_best: Some(20),
            beat_record_target: None,
            is_timed: false,
            confidence: GoalConfidence::High,
            fatigue_factor: 0.0,
            similar_sessions: 5,
            today_sets: 0,
            today_value: 0,
            fatigued_muscles: vec![],
            avg_7_days: Some(19.0),
            avg_14_days: Some(18.0),
            record_date: Some(Utc::now() - chrono::Duration::days(2)),
            is_consolidating: true,
            consolidation_days_left: Some(5),
            record_confirmed: false,
        };

        let formatted = goal.format_short();
        assert!(formatted.contains("Рекорд: 20 (закрепляем, 5 дн.)"), "Short format: {}", formatted);
        assert!(!formatted.contains("побей"), "Should not contain 'побей': {}", formatted);
    }

    #[test]
    fn test_consolidation_timed_exercise() {
        // Timed exercise (plank) - record 3 days ago
        let mut training = create_training("стойка на локтях", 1, 3);
        training.duration_secs = Some(120); // 2 minutes

        let trainings = vec![training];
        let goal = GoalCalculator::calculate(&trainings, "стойка на локтях");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(g.is_consolidating, "Timed exercise should also consolidate");
        assert!(g.is_timed);
        assert_eq!(g.personal_best, Some(120));
        assert_eq!(g.consolidation_days_left, Some(4)); // 7 - 3 = 4 days left
        assert!(g.record_confirmed); // Record was confirmed on the same day it was set

        let formatted = g.format();
        assert!(formatted.contains("2м (закрепляем, 4 дн.)"), "Timed format: {}", formatted);
    }

    // ===== New tests for enhanced consolidation =====

    #[test]
    fn test_consolidation_confirmed_unlocks() {
        // Record set 10 days ago, confirmed 3 days ago → should unlock challenge
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 10), // Record set 10 days ago
            create_training("отжимания на кулаках", 20, 3),  // Confirmed 3 days ago
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(!g.is_consolidating, "Should unlock after confirmation in window");
        assert_eq!(g.beat_record_target, Some(21));
        assert!(g.record_confirmed);
    }

    #[test]
    fn test_consolidation_not_confirmed_extends() {
        // Record set 10 days ago, never confirmed since → extend consolidation
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 10), // Record 10 days ago
            create_training("отжимания на кулаках", 15, 5),  // Below record
            create_training("отжимания на кулаках", 18, 2),  // Below record
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(g.is_consolidating, "Should extend consolidation if not confirmed");
        assert!(g.beat_record_target.is_none());
        assert!(!g.record_confirmed);
        assert!(g.consolidation_days_left.is_some());
    }

    #[test]
    fn test_consolidation_days_countdown() {
        // Record set 2 days ago → 5 days left
        let trainings = vec![
            create_training("отжимания на кулаках", 20, 2),
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert!(g.is_consolidating);
        assert_eq!(g.consolidation_days_left, Some(5)); // 7 - 2 = 5
    }

    #[test]
    fn test_consolidation_new_record_resets() {
        // Old record, then new record yesterday → should consolidate new record
        let trainings = vec![
            create_training("отжимания на кулаках", 15, 10), // Old record
            create_training("отжимания на кулаках", 20, 1),  // New record yesterday
        ];
        let goal = GoalCalculator::calculate(&trainings, "отжимания на кулаках");
        assert!(goal.is_some());
        let g = goal.unwrap();
        assert_eq!(g.personal_best, Some(20));
        assert!(g.is_consolidating, "Should consolidate new record");
        assert_eq!(g.consolidation_days_left, Some(6)); // 7 - 1 = 6
    }
}
