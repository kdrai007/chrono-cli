use chrono::{Duration, NaiveDate, TimeZone, Utc};
use clockify_tui::domain::{
    calculate_streaks, DailyStudySummary, EntryMode, PomodoroStats, Project, ProjectTargetProgress,
    StreakStats, SubjectBreakdown, TimeEntry,
};
use clockify_tui::storage::{Database, Repository};

#[test]
fn test_calculate_streaks_empty_history() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    let stats = calculate_streaks(&[], today);

    assert_eq!(
        stats,
        StreakStats {
            current_streak: 0,
            longest_streak: 0,
            studied_today: false,
            last_study_date: None,
        }
    );
}

#[test]
fn test_calculate_streaks_studied_today_consecutive() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    let dates = vec![today - Duration::days(2), today - Duration::days(1), today];

    let stats = calculate_streaks(&dates, today);

    assert_eq!(stats.current_streak, 3);
    assert_eq!(stats.longest_streak, 3);
    assert!(stats.studied_today);
    assert_eq!(stats.last_study_date, Some(today));
}

#[test]
fn test_calculate_streaks_preserved_today_until_midnight() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    // Haven't studied today yet, but studied yesterday and 2 days ago
    let dates = vec![today - Duration::days(2), today - Duration::days(1)];

    let stats = calculate_streaks(&dates, today);

    assert_eq!(stats.current_streak, 2);
    assert_eq!(stats.longest_streak, 2);
    assert!(!stats.studied_today);
    assert_eq!(stats.last_study_date, Some(today - Duration::days(1)));
}

#[test]
fn test_calculate_streaks_missed_yesterday_and_today() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    // Studied 2 days ago, but missed yesterday and today -> current streak broken
    let dates = vec![today - Duration::days(2)];

    let stats = calculate_streaks(&dates, today);

    assert_eq!(stats.current_streak, 0);
    assert_eq!(stats.longest_streak, 1);
    assert!(!stats.studied_today);
    assert_eq!(stats.last_study_date, Some(today - Duration::days(2)));
}

#[test]
fn test_calculate_streaks_multi_gap_history() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
    // 4 consecutive days: days 1..=4
    // 2 missed days: days 5, 6
    // 2 consecutive days: days 7, 8 (which are yesterday and today relative to day 8,
    // or ending yesterday relative to day 9, or ending 2 days ago relative to day 10)
    let day1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let day2 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    let day3 = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let day4 = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    // days 5, 6 skipped
    let day7 = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
    let day8 = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();

    let dates = vec![day1, day2, day3, day4, day7, day8];

    // If today is day 8: current = 2, longest = 4
    let stats_day8 = calculate_streaks(&dates, day8);
    assert_eq!(stats_day8.current_streak, 2);
    assert_eq!(stats_day8.longest_streak, 4);
    assert!(stats_day8.studied_today);

    // If today is day 9 (haven't studied yet on day 9, but studied on day 8): current = 2, longest = 4
    let day9 = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let stats_day9 = calculate_streaks(&dates, day9);
    assert_eq!(stats_day9.current_streak, 2);
    assert_eq!(stats_day9.longest_streak, 4);
    assert!(!stats_day9.studied_today);

    // If today is day 10 (missed day 9 and day 10): current = 0, longest = 4
    let stats_day10 = calculate_streaks(&dates, today);
    assert_eq!(stats_day10.current_streak, 0);
    assert_eq!(stats_day10.longest_streak, 4);
    assert!(!stats_day10.studied_today);
}

#[test]
fn test_calculate_streaks_duplicate_and_unsorted_dates() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
    let yesterday = today - Duration::days(1);
    // Multiple sessions on the same date count as 1 study day, and unsorted order is handled
    let dates = vec![
        today,
        yesterday,
        today,
        yesterday,
        today - Duration::days(2),
        today,
    ];

    let stats = calculate_streaks(&dates, today);
    assert_eq!(stats.current_streak, 3);
    assert_eq!(stats.longest_streak, 3);
    assert!(stats.studied_today);
}

#[test]
fn test_db_get_streak_stats() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();

    // Empty DB streak
    let empty_stats = db.get_streak_stats(today).unwrap();
    assert_eq!(empty_stats.current_streak, 0);
    assert_eq!(empty_stats.longest_streak, 0);
    assert!(!empty_stats.studied_today);

    // Insert entries for yesterday and today
    let yesterday_dt = Utc.with_ymd_and_hms(2026, 10, 3, 14, 0, 0).unwrap();
    let today_dt1 = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let today_dt2 = Utc.with_ymd_and_hms(2026, 10, 4, 16, 0, 0).unwrap();

    db.create_manual_entry(
        &TimeEntry::new("Study yesterday", yesterday_dt)
            .with_end_time(yesterday_dt + Duration::hours(1)),
    )
    .unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Study today session 1", today_dt1)
            .with_end_time(today_dt1 + Duration::minutes(45)),
    )
    .unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Study today session 2", today_dt2)
            .with_end_time(today_dt2 + Duration::minutes(30)),
    )
    .unwrap();

    let stats = db.get_streak_stats(today).unwrap();
    assert_eq!(stats.current_streak, 2);
    assert_eq!(stats.longest_streak, 2);
    assert!(stats.studied_today);
    assert_eq!(stats.last_study_date, Some(today));
}

#[test]
fn test_db_get_daily_summaries() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let day1 = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let _day2 = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    let day3 = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

    let dt1_1 = Utc.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap();
    let dt1_2 = Utc.with_ymd_and_hms(2026, 10, 1, 14, 0, 0).unwrap();
    let dt3_1 = Utc.with_ymd_and_hms(2026, 10, 3, 11, 0, 0).unwrap();

    // Day 1: 1h + 30m = 90m
    db.create_manual_entry(
        &TimeEntry::new("Morning study", dt1_1).with_end_time(dt1_1 + Duration::hours(1)),
    )
    .unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Afternoon study", dt1_2).with_end_time(dt1_2 + Duration::minutes(30)),
    )
    .unwrap();
    // Day 2 has no entries
    // Day 3: 2h = 120m
    db.create_manual_entry(
        &TimeEntry::new("Day 3 study", dt3_1).with_end_time(dt3_1 + Duration::hours(2)),
    )
    .unwrap();

    let summaries = db.get_daily_summaries(day1, day3).unwrap();

    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[0],
        DailyStudySummary {
            date: day1,
            duration: Duration::minutes(90),
        }
    );
    assert_eq!(
        summaries[1],
        DailyStudySummary {
            date: day3,
            duration: Duration::hours(2),
        }
    );
}

#[test]
fn test_db_get_weekly_project_progress() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p1 = db
        .create_project(
            &Project::new("Algorithms")
                .unwrap()
                .with_target_hours_week(10.0)
                .with_color("#e74c3c"),
        )
        .unwrap();
    let p2 = db
        .create_project(
            &Project::new("Systems")
                .unwrap()
                .with_target_hours_week(5.0)
                .with_color("#2ecc71"),
        )
        .unwrap();
    let _p3 = db
        .create_project(
            &Project::new("Elective")
                .unwrap()
                .with_target_hours_week(0.0)
                .with_color("#9b59b6"),
        )
        .unwrap();

    let monday = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let sunday = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();

    // Log 7.5 hours for Algorithms (75% progress)
    let t1 = Utc.with_ymd_and_hms(2026, 10, 6, 10, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Graph theory", t1)
            .with_project_id(p1.id.unwrap())
            .with_end_time(t1 + Duration::minutes(450)), // 7.5h
    )
    .unwrap();

    // Log 6.0 hours for Systems (120% progress)
    let t2 = Utc.with_ymd_and_hms(2026, 10, 7, 13, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("OS kernel", t2)
            .with_project_id(p2.id.unwrap())
            .with_end_time(t2 + Duration::hours(6)),
    )
    .unwrap();

    let progress_list = db.get_weekly_project_progress(monday, sunday).unwrap();

    assert_eq!(progress_list.len(), 3);

    let algo = progress_list
        .iter()
        .find(|p| p.project_name == "Algorithms")
        .expect("Algorithms not found");
    assert_eq!(algo.project_id, p1.id.unwrap());
    assert_eq!(algo.color, "#e74c3c");
    assert!((algo.target_hours_week - 10.0).abs() < 1e-6);
    assert!((algo.actual_hours_week - 7.5).abs() < 1e-6);
    assert!((algo.progress_ratio - 0.75).abs() < 1e-6);
    assert!((algo.percentage() - 75.0).abs() < 1e-6);

    let systems = progress_list
        .iter()
        .find(|p| p.project_name == "Systems")
        .expect("Systems not found");
    assert_eq!(systems.project_id, p2.id.unwrap());
    assert_eq!(systems.color, "#2ecc71");
    assert!((systems.target_hours_week - 5.0).abs() < 1e-6);
    assert!((systems.actual_hours_week - 6.0).abs() < 1e-6);
    assert!((systems.progress_ratio - 1.2).abs() < 1e-6);
    assert!((systems.percentage() - 120.0).abs() < 1e-6);

    let elective = progress_list
        .iter()
        .find(|p| p.project_name == "Elective")
        .expect("Elective not found");
    assert!((elective.target_hours_week - 0.0).abs() < 1e-6);
    assert!((elective.actual_hours_week - 0.0).abs() < 1e-6);
    assert!((elective.progress_ratio - 0.0).abs() < 1e-6);
    assert!((elective.percentage() - 0.0).abs() < 1e-6);
}

#[test]
fn test_db_get_subject_breakdown() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p1 = db
        .create_project(&Project::new("Math").unwrap().with_color("#1abc9c"))
        .unwrap();
    let p2 = db
        .create_project(&Project::new("Physics").unwrap().with_color("#3498db"))
        .unwrap();

    let start_date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();

    let t1 = Utc.with_ymd_and_hms(2026, 10, 2, 9, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 10, 3, 10, 0, 0).unwrap();

    // Math: 3 hours
    db.create_manual_entry(
        &TimeEntry::new("Calculus", t1)
            .with_project_id(p1.id.unwrap())
            .with_end_time(t1 + Duration::hours(3)),
    )
    .unwrap();

    // Physics: 1 hour
    db.create_manual_entry(
        &TimeEntry::new("Mechanics", t2)
            .with_project_id(p2.id.unwrap())
            .with_end_time(t2 + Duration::hours(1)),
    )
    .unwrap();

    let breakdown = db.get_subject_breakdown(start_date, end_date).unwrap();

    assert_eq!(breakdown.len(), 2);
    // Total is 4 hours. Math is 3h (75%), Physics is 1h (25%)
    assert_eq!(breakdown[0].project_name, "Math");
    assert_eq!(breakdown[0].duration, Duration::hours(3));
    assert!((breakdown[0].percentage - 75.0).abs() < 1e-6);
    assert_eq!(breakdown[0].color, "#1abc9c");

    assert_eq!(breakdown[1].project_name, "Physics");
    assert_eq!(breakdown[1].duration, Duration::hours(1));
    assert!((breakdown[1].percentage - 25.0).abs() < 1e-6);
    assert_eq!(breakdown[1].color, "#3498db");
}

#[test]
fn test_db_get_pomodoro_stats() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let monday = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let wednesday = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();

    // Session on Monday (this week, but not today): 25 mins completed
    let t_mon = Utc.with_ymd_and_hms(2026, 10, 5, 10, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Monday pomo 1", t_mon)
            .with_entry_mode(EntryMode::PomodoroWork)
            .with_end_time(t_mon + Duration::minutes(25)),
    )
    .unwrap();

    // Session on Wednesday (today): 25 mins completed
    let t_wed1 = Utc.with_ymd_and_hms(2026, 10, 7, 11, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Wednesday pomo 1", t_wed1)
            .with_entry_mode(EntryMode::PomodoroWork)
            .with_end_time(t_wed1 + Duration::minutes(25)),
    )
    .unwrap();

    // Session on Wednesday (today): 25 mins completed
    let t_wed2 = Utc.with_ymd_and_hms(2026, 10, 7, 14, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Wednesday pomo 2", t_wed2)
            .with_entry_mode(EntryMode::PomodoroWork)
            .with_end_time(t_wed2 + Duration::minutes(25)),
    )
    .unwrap();

    // Break on Wednesday: should NOT count as pomodoro work
    let t_break = Utc.with_ymd_and_hms(2026, 10, 7, 11, 25, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Short break", t_break)
            .with_entry_mode(EntryMode::PomodoroBreak)
            .with_end_time(t_break + Duration::minutes(5)),
    )
    .unwrap();

    // Incomplete/active pomo session: should NOT count as completed
    let t_active = Utc.with_ymd_and_hms(2026, 10, 7, 16, 0, 0).unwrap();
    db.start_entry(
        &TimeEntry::new("Active pomo", t_active).with_entry_mode(EntryMode::PomodoroWork),
    )
    .unwrap();

    let stats: PomodoroStats = db.get_pomodoro_stats(wednesday, monday).unwrap();

    assert_eq!(stats.completed_today, 2);
    assert_eq!(stats.completed_this_week, 3);
    assert_eq!(stats.total_focus_mins, 75); // 3 * 25m = 75m
}

#[test]
fn test_repository_trait_stats_dispatch() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");
    let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();

    let p = db
        .create_project(&Project::new("Study").unwrap().with_target_hours_week(10.0))
        .unwrap();

    let t = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    db.create_manual_entry(
        &TimeEntry::new("Session", t)
            .with_project_id(p.id.unwrap())
            .with_entry_mode(EntryMode::PomodoroWork)
            .with_end_time(t + Duration::minutes(25)),
    )
    .unwrap();

    // Verify polymorphic dispatch through &dyn Repository
    let repo: &dyn Repository = &db;
    let streak: StreakStats = repo.get_streak_stats(today).unwrap();
    assert_eq!(streak.current_streak, 1);

    let summaries: Vec<DailyStudySummary> = repo.get_daily_summaries(today, today).unwrap();
    assert_eq!(summaries.len(), 1);

    let progress: Vec<ProjectTargetProgress> =
        repo.get_weekly_project_progress(today, today).unwrap();
    assert_eq!(progress.len(), 1);

    let breakdown: Vec<SubjectBreakdown> = repo.get_subject_breakdown(today, today).unwrap();
    assert_eq!(breakdown.len(), 1);

    let pomo: PomodoroStats = repo.get_pomodoro_stats(today, today).unwrap();
    assert_eq!(pomo.completed_today, 1);
}
