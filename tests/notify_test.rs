use std::sync::Arc;

use chrono_cli::config::GeneralConfig;
use chrono_cli::domain::PomodoroPhase;
use chrono_cli::notify::{
    MockNotificationSink, NotificationEvent, NotificationService, NotificationUrgency, NotifyError,
};

#[test]
fn test_notification_event_pomodoro_work_to_short_break() {
    let event = NotificationEvent::PomodoroFinished {
        phase: PomodoroPhase::Work(1),
        next_phase: PomodoroPhase::ShortBreak(1),
    };

    assert_eq!(event.title(), "Focus Session Complete!");
    assert_eq!(event.body(), "Time for a short break.");
    assert_eq!(event.urgency(), NotificationUrgency::Normal);
}

#[test]
fn test_notification_event_pomodoro_work_to_long_break() {
    let event = NotificationEvent::PomodoroFinished {
        phase: PomodoroPhase::Work(4),
        next_phase: PomodoroPhase::LongBreak(4),
    };

    assert_eq!(event.title(), "Focus Session Complete!");
    assert!(
        event.body().contains("long break") || event.body().contains("15-minute break"),
        "Body was: {}",
        event.body()
    );
    assert_eq!(event.urgency(), NotificationUrgency::Normal);
}

#[test]
fn test_notification_event_pomodoro_break_to_work() {
    let event = NotificationEvent::PomodoroFinished {
        phase: PomodoroPhase::ShortBreak(1),
        next_phase: PomodoroPhase::Work(2),
    };

    assert!(
        event.title().contains("Break"),
        "Title was: {}",
        event.title()
    );
    assert!(
        event.body().contains("focus") || event.body().contains("work"),
        "Body was: {}",
        event.body()
    );
    assert_eq!(event.urgency(), NotificationUrgency::Normal);
}

#[test]
fn test_notification_event_study_target_reached() {
    let event = NotificationEvent::StudyTargetReached {
        project_name: "Mathematics".to_string(),
        target_hours: 4.0,
    };

    assert_eq!(event.title(), "Study Target Reached!");
    assert!(event.body().contains("Mathematics"));
    assert!(event.body().contains("4"));
    assert_eq!(event.urgency(), NotificationUrgency::Normal);
}

#[test]
fn test_notification_event_sync_completed_success() {
    let event = NotificationEvent::SyncCompleted {
        success: true,
        message: "10 entries pushed".to_string(),
    };

    assert_eq!(event.title(), "Sync Successful");
    assert_eq!(event.body(), "10 entries pushed");
    assert_eq!(event.urgency(), NotificationUrgency::Low);
}

#[test]
fn test_notification_event_sync_completed_failure() {
    let event = NotificationEvent::SyncCompleted {
        success: false,
        message: "Server 500 error".to_string(),
    };

    assert_eq!(event.title(), "Sync Failed");
    assert_eq!(event.body(), "Server 500 error");
    assert_eq!(event.urgency(), NotificationUrgency::Critical);
}

#[test]
fn test_notification_event_custom() {
    let event = NotificationEvent::Custom {
        title: "Custom Alert".to_string(),
        body: "Custom details".to_string(),
    };

    assert_eq!(event.title(), "Custom Alert");
    assert_eq!(event.body(), "Custom details");
    assert_eq!(event.urgency(), NotificationUrgency::Normal);
}

#[test]
fn test_service_desktop_notifications_disabled() {
    let config = GeneralConfig {
        desktop_notifications: false,
        terminal_bell: true,
        ..Default::default()
    };
    let (service, mock) = NotificationService::in_memory(config);

    let res = service.send_desktop("Should skip", "Body");
    assert!(res.is_ok());
    assert_eq!(mock.sent_notifications().len(), 0);

    let res = service.send(&NotificationEvent::Custom {
        title: "Title".to_string(),
        body: "Body".to_string(),
    });
    assert!(res.is_ok());
    assert_eq!(mock.sent_notifications().len(), 0);
    // Bell was enabled, so it should have rung once
    assert_eq!(mock.bell_count(), 1);
}

#[test]
fn test_service_terminal_bell_disabled() {
    let config = GeneralConfig {
        desktop_notifications: true,
        terminal_bell: false,
        ..Default::default()
    };
    let (service, mock) = NotificationService::in_memory(config);

    service.ring_bell();
    assert_eq!(mock.bell_count(), 0);

    let res = service.send(&NotificationEvent::Custom {
        title: "Title".to_string(),
        body: "Body".to_string(),
    });
    assert!(res.is_ok());
    assert_eq!(mock.bell_count(), 0);
    assert_eq!(mock.sent_notifications().len(), 1);
}

#[test]
fn test_service_both_enabled() {
    let config = GeneralConfig {
        desktop_notifications: true,
        terminal_bell: true,
        ..Default::default()
    };
    let (service, mock) = NotificationService::in_memory(config);

    service.ring_bell();
    assert_eq!(mock.bell_count(), 1);

    let event = NotificationEvent::Custom {
        title: "Test".to_string(),
        body: "Desc".to_string(),
    };
    let res = service.send(&event);
    assert!(res.is_ok());
    assert_eq!(mock.bell_count(), 2);
    assert_eq!(mock.sent_notifications().len(), 1);
    assert_eq!(mock.sent_notifications()[0].0, "Test");
    assert_eq!(mock.sent_notifications()[0].1, "Desc");
    assert_eq!(mock.sent_notifications()[0].2, NotificationUrgency::Normal);
}

#[test]
fn test_service_event_recording() {
    let (service, _mock) = NotificationService::in_memory(GeneralConfig::default());
    assert_eq!(service.recorded_events().len(), 0);

    let event1 = NotificationEvent::Custom {
        title: "E1".to_string(),
        body: "B1".to_string(),
    };
    let event2 = NotificationEvent::Custom {
        title: "E2".to_string(),
        body: "B2".to_string(),
    };

    service.send(&event1).unwrap();
    service.send(&event2).unwrap();

    assert_eq!(service.recorded_events(), vec![event1, event2]);

    service.clear_recorded_events();
    assert_eq!(service.recorded_events().len(), 0);
}

#[test]
fn test_service_convenience_methods() {
    let (service, mock) = NotificationService::in_memory(GeneralConfig::default());

    service
        .notify_pomodoro_phase_change(&PomodoroPhase::Work(1), &PomodoroPhase::ShortBreak(1))
        .unwrap();
    service.notify_target_reached("Physics", 2.5).unwrap();
    service.notify_sync(true, "All synced").unwrap();

    assert_eq!(mock.sent_notifications().len(), 3);
    assert_eq!(mock.bell_count(), 3);
    assert_eq!(service.recorded_events().len(), 3);

    assert_eq!(mock.sent_notifications()[0].0, "Focus Session Complete!");
    assert_eq!(mock.sent_notifications()[1].0, "Study Target Reached!");
    assert_eq!(mock.sent_notifications()[2].0, "Sync Successful");
}

#[test]
fn test_service_mock_sink_error_handling() {
    let (service, mock) = NotificationService::in_memory(GeneralConfig::default());
    mock.set_fail_desktop(true);

    let res = service.send_desktop("Fail", "Fail");
    assert!(matches!(res, Err(NotifyError::DesktopNotification(_))));

    let event = NotificationEvent::Custom {
        title: "T".to_string(),
        body: "B".to_string(),
    };
    let res = service.send(&event);
    assert!(matches!(res, Err(NotifyError::DesktopNotification(_))));
}

#[test]
#[ignore = "Avoid sending real desktop notification popup during automated unit tests"]
fn test_service_headless_real_sink_no_panic() {
    let service = NotificationService::new(GeneralConfig {
        desktop_notifications: true,
        terminal_bell: false,
        ..Default::default()
    });

    // In CI / headless environment, D-Bus may be missing. notify() must NOT panic.
    let res = service.notify(&NotificationEvent::Custom {
        title: "Headless".to_string(),
        body: "Safe".to_string(),
    });
    // We don't assert res.is_ok(), but verify that calling notify did not panic:
    let _ = res;

    let res_desktop = service.send_desktop("Headless Direct", "Safe");
    let _ = res_desktop;

    // Also verify with desktop_notifications disabled on real service:
    let service_disabled = NotificationService::new(GeneralConfig {
        desktop_notifications: false,
        terminal_bell: false,
        ..Default::default()
    });
    assert!(service_disabled.send_desktop("Skipped", "Safe").is_ok());
    assert!(service_disabled
        .notify(&NotificationEvent::Custom {
            title: "Skipped".to_string(),
            body: "Safe".to_string()
        })
        .is_ok());
}

#[test]
fn test_service_with_custom_sink_and_clear() {
    let mock = Arc::new(MockNotificationSink::new());
    let service = NotificationService::with_sink(GeneralConfig::default(), mock.clone());

    service.send_desktop("Title", "Body").unwrap();
    service.ring_bell();

    assert_eq!(mock.sent_notifications().len(), 1);
    assert_eq!(mock.bell_count(), 1);

    mock.clear();
    assert_eq!(mock.sent_notifications().len(), 0);
    assert_eq!(mock.bell_count(), 0);
}

#[test]
fn test_service_convenience_aliases() {
    let (service, mock) = NotificationService::in_memory(GeneralConfig::default());

    service
        .notify_pomodoro_completed(&PomodoroPhase::Work(1), &PomodoroPhase::ShortBreak(1))
        .unwrap();
    service.notify_sync_status(true, "Synced 3 items").unwrap();

    let sent = mock.sent_notifications();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].0, "Focus Session Complete!");
    assert_eq!(sent[1].0, "Sync Successful");
}
