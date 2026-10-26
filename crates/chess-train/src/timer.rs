//! Time limit of a training run: raises its stop flag once a deadline
//! passes, which ends the run cleanly (current step, checkpoint, export).
//! The deadline can be set, moved or removed while the run goes on.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use chess_search::StopFlag;

use crate::metrics::now;

/// Longest time limit: a week.
pub(crate) const MAX_MINUTES: u64 = 7 * 24 * 60;

/// How often the watcher looks at the clock.
const TICK: Duration = Duration::from_millis(500);

/// A shared, changeable deadline (Unix seconds).
#[derive(Debug, Clone, Default)]
pub(crate) struct Timer {
    deadline: Arc<Mutex<Option<u64>>>,
}

impl Timer {
    /// Why `minutes` is not an accepted time limit, if it is not.
    pub(crate) fn validate(minutes: u64) -> Result<(), String> {
        if (1..=MAX_MINUTES).contains(&minutes) {
            Ok(())
        } else {
            Err(format!(
                "the time limit must be between 1 and {MAX_MINUTES} minutes"
            ))
        }
    }

    /// Stops `minutes` from now; `None`: no limit.
    pub(crate) fn set(&self, minutes: Option<u64>) {
        let deadline = minutes.map(|minutes| now() + minutes * 60);
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = deadline;
        match deadline {
            Some(deadline) => tracing::info!(deadline, minutes, "training time limit set"),
            None => tracing::info!("training time limit removed"),
        }
    }

    /// Unix seconds at which the run stops, if limited.
    pub(crate) fn deadline(&self) -> Option<u64> {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Raises `stop` once the deadline passes. Returns when `stop` is
    /// raised, by the deadline or by anyone else.
    pub(crate) fn watch(&self, stop: StopFlag) -> JoinHandle<()> {
        let timer = self.clone();
        std::thread::spawn(move || {
            while !stop.is_stopped() {
                if timer.deadline().is_some_and(|deadline| now() >= deadline) {
                    tracing::info!("training time limit reached: stopping after the current step");
                    stop.stop();
                    break;
                }
                std::thread::sleep(TICK);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_validated() {
        assert!(Timer::validate(1).is_ok());
        assert!(Timer::validate(MAX_MINUTES).is_ok());
        assert!(Timer::validate(0).is_err());
        assert!(Timer::validate(MAX_MINUTES + 1).is_err());
    }

    #[test]
    fn deadline_can_be_moved_and_raises_the_flag() {
        let timer = Timer::default();
        assert_eq!(timer.deadline(), None);
        timer.set(Some(60));
        let deadline = timer.deadline().unwrap();
        assert!(deadline >= now() + 3_590 && deadline <= now() + 3_600);

        let stop = StopFlag::new();
        let watcher = timer.watch(stop.clone());
        std::thread::sleep(TICK * 2);
        assert!(!stop.is_stopped(), "an hour is not over yet");
        // Moved into the past: the watcher stops the run.
        *timer.deadline.lock().unwrap() = Some(now() - 1);
        watcher.join().unwrap();
        assert!(stop.is_stopped());

        // A run stopped by other means releases its watcher.
        let other = StopFlag::new();
        let timer = Timer::default();
        let watcher = timer.watch(other.clone());
        other.stop();
        watcher.join().unwrap();
        timer.set(None);
        assert_eq!(timer.deadline(), None);
    }
}
