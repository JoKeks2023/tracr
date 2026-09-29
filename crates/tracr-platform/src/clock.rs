//! Clocks. The core never reads a clock; everything that needs "now" gets a [`Clock`].

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracr_core::time::UtcMillis;

pub trait Clock: Send + Sync {
    fn now(&self) -> UtcMillis;
}

/// Wall-clock time of this machine.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UtcMillis {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        UtcMillis(millis)
    }
}

/// A clock that only moves when told to (scenario time in tests).
#[derive(Debug)]
pub struct ManualClock(AtomicI64);

impl ManualClock {
    pub fn new(start: UtcMillis) -> Self {
        Self(AtomicI64::new(start.0))
    }

    pub fn set(&self, at: UtcMillis) {
        self.0.store(at.0, Ordering::SeqCst);
    }

    pub fn advance_ms(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> UtcMillis {
        UtcMillis(self.0.load(Ordering::SeqCst))
    }
}
