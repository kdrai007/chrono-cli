use std::collections::HashSet;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

/// Summary of daily study time for a specific calendar date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyStudySummary {
    /// The calendar date.
    pub date: NaiveDate,
    /// Total duration of study recorded on this date.
    pub duration: Duration,
}

/// Statistics for study streak tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreakStats {
    /// Current consecutive days studied. Preserved until midnight if not studied yet today.
    pub current_streak: u32,
    /// All-time longest streak in days.
    pub longest_streak: u32,
    /// Whether the user has studied on `today`.
    pub studied_today: bool,
    /// Most recent study date recorded on or before `today`.
    pub last_study_date: Option<NaiveDate>,
}

/// Progress metrics for a project against its weekly study target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTargetProgress {
    /// Database ID of the project.
    pub project_id: i64,
    /// Project display name.
    pub project_name: String,
    /// Hex color code for UI rendering.
    pub color: String,
    /// Target study hours for the week.
    pub target_hours_week: f64,
    /// Actual hours logged for this project during the week.
    pub actual_hours_week: f64,
    /// Ratio of actual to target hours (0.0 to 1.0+).
    pub progress_ratio: f64,
}

impl ProjectTargetProgress {
    /// Creates a new `ProjectTargetProgress` instance, calculating `progress_ratio`.
    pub fn new(
        project_id: i64,
        project_name: impl Into<String>,
        color: impl Into<String>,
        target_hours_week: f64,
        actual_hours_week: f64,
    ) -> Self {
        let progress_ratio = if target_hours_week > 0.0 {
            actual_hours_week / target_hours_week
        } else {
            0.0
        };

        Self {
            project_id,
            project_name: project_name.into(),
            color: color.into(),
            target_hours_week,
            actual_hours_week,
            progress_ratio,
        }
    }

    /// Target hours for the week.
    pub fn target_hours(&self) -> f64 {
        self.target_hours_week
    }

    /// Actual hours logged this week.
    pub fn actual_hours(&self) -> f64 {
        self.actual_hours_week
    }

    /// Progress percentage (0.0 to 100.0+).
    pub fn percentage(&self) -> f64 {
        self.progress_ratio * 100.0
    }
}

/// Breakdown of study time spent on a project within a date range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubjectBreakdown {
    /// Name of the project / subject.
    pub project_name: String,
    /// Hex color code for UI badges.
    pub color: String,
    /// Total duration logged for this project in the period.
    pub duration: Duration,
    /// Percentage of total study time in the period (0.0 to 100.0).
    pub percentage: f64,
}

impl SubjectBreakdown {
    /// Creates a new `SubjectBreakdown`.
    pub fn new(
        project_name: impl Into<String>,
        color: impl Into<String>,
        duration: Duration,
        percentage: f64,
    ) -> Self {
        Self {
            project_name: project_name.into(),
            color: color.into(),
            duration,
            percentage,
        }
    }
}

/// Aggregated Pomodoro work session statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PomodoroStats {
    /// Number of completed pomodoro work sessions on `today`.
    pub completed_today: u32,
    /// Number of completed pomodoro work sessions during the week starting at `week_start`.
    pub completed_this_week: u32,
    /// Total minutes of completed pomodoro work sessions during the week.
    pub total_focus_mins: i64,
}

/// Calculates study streak metrics given a slice of study dates and a reference `today` date.
///
/// Rules:
/// - Duplicate dates in `dates` count as a single study day.
/// - Unsorted dates are handled properly.
/// - Dates in the future (relative to `today`) are ignored.
/// - If studied today, `current_streak` counts consecutive days backwards from today.
/// - If not studied today, but studied yesterday, the streak is preserved until midnight.
/// - If not studied today and not studied yesterday, `current_streak` is 0.
/// - `longest_streak` is the maximum consecutive run of days anywhere in the history up to `today`.
pub fn calculate_streaks(dates: &[NaiveDate], today: NaiveDate) -> StreakStats {
    if dates.is_empty() {
        return StreakStats {
            current_streak: 0,
            longest_streak: 0,
            studied_today: false,
            last_study_date: None,
        };
    }

    let mut unique_dates: Vec<NaiveDate> = dates.iter().copied().filter(|&d| d <= today).collect();
    unique_dates.sort_unstable();
    unique_dates.dedup();

    if unique_dates.is_empty() {
        return StreakStats {
            current_streak: 0,
            longest_streak: 0,
            studied_today: false,
            last_study_date: None,
        };
    }

    let last_study_date = unique_dates.last().copied();
    let studied_today = last_study_date == Some(today);

    let date_set: HashSet<NaiveDate> = unique_dates.iter().copied().collect();

    let current_streak = if studied_today {
        let mut count = 0;
        let mut curr = today;
        while date_set.contains(&curr) {
            count += 1;
            curr -= Duration::days(1);
        }
        count
    } else {
        let yesterday = today - Duration::days(1);
        if date_set.contains(&yesterday) {
            let mut count = 0;
            let mut curr = yesterday;
            while date_set.contains(&curr) {
                count += 1;
                curr -= Duration::days(1);
            }
            count
        } else {
            0
        }
    };

    let mut longest_streak = 1;
    let mut current_run = 1;
    for i in 1..unique_dates.len() {
        if unique_dates[i] == unique_dates[i - 1] + Duration::days(1) {
            current_run += 1;
            if current_run > longest_streak {
                longest_streak = current_run;
            }
        } else {
            current_run = 1;
        }
    }

    if current_streak > longest_streak {
        longest_streak = current_streak;
    }

    StreakStats {
        current_streak,
        longest_streak,
        studied_today,
        last_study_date,
    }
}
