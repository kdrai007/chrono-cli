//! Event handling and asynchronous crossterm event stream.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};

/// Unified application event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Terminal keyboard key press.
    Key(KeyEvent),
    /// Periodic tick event for timer updates and animation.
    Tick,
    /// Terminal window resize event with new width and height.
    Resize(u16, u16),
}

/// Asynchronous event handler polling crossterm events in a background thread.
pub struct EventHandler {
    receiver: mpsc::Receiver<Event>,
    sender: mpsc::Sender<Event>,
    _handler: Option<thread::JoinHandle<()>>,
}

impl EventHandler {
    /// Default tick rate (250 milliseconds).
    pub const DEFAULT_TICK_RATE: Duration = Duration::from_millis(250);

    /// Spawns a background thread that polls crossterm events at the given tick rate.
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::channel();
        let event_sender = sender.clone();

        let handler = thread::Builder::new()
            .name("clockify-tui-events".to_string())
            .spawn(move || loop {
                match event::poll(tick_rate) {
                    Ok(true) => match event::read() {
                        Ok(CrosstermEvent::Key(key))
                            if (key.kind == KeyEventKind::Press
                                || key.kind == KeyEventKind::Repeat)
                                && event_sender.send(Event::Key(key)).is_err() =>
                        {
                            break;
                        }
                        Ok(CrosstermEvent::Resize(w, h))
                            if event_sender.send(Event::Resize(w, h)).is_err() =>
                        {
                            break;
                        }
                        _ => {}
                    },
                    Ok(false) => {
                        if event_sender.send(Event::Tick).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            })
            .ok();

        Self {
            receiver,
            sender,
            _handler: handler,
        }
    }

    /// Receives the next event, blocking until an event is available.
    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.receiver.recv()
    }

    /// Attempts to receive the next event without blocking.
    pub fn try_next(&self) -> Result<Event, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Returns a clone of the event channel sender.
    pub fn sender(&self) -> mpsc::Sender<Event> {
        self.sender.clone()
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new(Self::DEFAULT_TICK_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn test_event_handler_channel_manual_send() {
        let (sender, receiver) = mpsc::channel();
        let handler = EventHandler {
            receiver,
            sender: sender.clone(),
            _handler: None,
        };

        sender.send(Event::Tick).unwrap();
        let received = handler.next().unwrap();
        assert_eq!(received, Event::Tick);

        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        sender.send(Event::Key(key)).unwrap();
        let received = handler.next().unwrap();
        assert_eq!(received, Event::Key(key));

        sender.send(Event::Resize(80, 24)).unwrap();
        let received = handler.next().unwrap();
        assert_eq!(received, Event::Resize(80, 24));
    }
}
