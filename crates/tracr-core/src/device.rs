//! Device evidence from OS samples (architecture §4.8, activity model §6).
//!
//! The platform sensor returns raw samples: idle duration, lock state and
//! the kernel's last sleep and wake times. [`DeviceTracker`] turns successive
//! samples into transition facts. It is pure: time comes from the samples.
//!
//! * Idle is recorded at a fixed **collection threshold** (60 s). The idle
//!   transition is placed at the last input instant (`sample − idle`), which
//!   is observed. The configured idle threshold (AD-13) is applied later by
//!   the human timeline, never here.
//! * The input that ends an idle period happened between the previous
//!   sample and `sample − idle`: `bounded`.
//! * Lock changes are seen at a sample: `bounded` since the previous sample.
//! * Sleep and wake come from the kernel's own times: `observed`.
//!
//! Only device state is recorded: no application, window, keystroke or
//! screen content (PS-01).

use serde::{Deserialize, Serialize};

use crate::events::FactKind;
use crate::time::UtcMillis;

pub const COLLECTION_THRESHOLD_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceEvent {
    Active,
    Idle,
    Locked,
    Unlocked,
    Suspended,
    Resumed,
}

impl DeviceEvent {
    pub fn kind(&self) -> FactKind {
        match self {
            Self::Active => FactKind::DeviceActive,
            Self::Idle => FactKind::DeviceIdle,
            Self::Locked => FactKind::DeviceLocked,
            Self::Unlocked => FactKind::DeviceUnlocked,
            Self::Suspended => FactKind::DeviceSuspended,
            Self::Resumed => FactKind::DeviceResumed,
        }
    }
}

/// One device transition as retained evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFacts {
    pub event: DeviceEvent,
    /// Where the transition is placed (the latest possible instant when bounded).
    pub at: UtcMillis,
    /// The earliest possible instant when the transition is only bounded by samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest: Option<UtcMillis>,
    /// `observed` or `bounded` (activity model §4).
    pub basis: String,
    pub sample_at: UtcMillis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_threshold_ms: Option<i64>,
    /// Which OS signal produced it (e.g. `hid_idle_time`, `session_lock`, `kernel_sleep_wake`).
    pub signal: String,
}

/// A raw sample from the platform sensor. `None` means the signal is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSample {
    pub at: UtcMillis,
    pub idle_ms: Option<i64>,
    pub locked: Option<bool>,
    /// The kernel's last (sleep, wake) times.
    pub last_sleep_wake: Option<(UtcMillis, UtcMillis)>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceTracker {
    idle: Option<bool>,
    locked: Option<bool>,
    last_sample: Option<UtcMillis>,
    last_wake: Option<UtcMillis>,
}

fn facts(
    event: DeviceEvent,
    at: UtcMillis,
    earliest: Option<UtcMillis>,
    sample: &DeviceSample,
    signal: &str,
) -> DeviceFacts {
    DeviceFacts {
        event,
        at,
        basis: if earliest.is_some() {
            "bounded"
        } else {
            "observed"
        }
        .to_owned(),
        earliest,
        sample_at: sample.at,
        collection_threshold_ms: matches!(event, DeviceEvent::Active | DeviceEvent::Idle)
            .then_some(COLLECTION_THRESHOLD_MS),
        signal: signal.to_owned(),
    }
}

impl DeviceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Transitions since the previous sample, in time order. The first
    /// sample reports the initial idle/active state (and a lock, if locked).
    pub fn observe(&mut self, s: &DeviceSample) -> Vec<DeviceFacts> {
        let mut out = Vec::new();
        let prev = self.last_sample;

        if let Some((sleep, wake)) = s.last_sleep_wake {
            let new_wake = match (self.last_wake, prev) {
                (Some(known), _) => wake > known,
                // Only a wake after this tracker started is a new transition.
                (None, Some(p)) => wake > p,
                (None, None) => false,
            };
            if new_wake && sleep <= wake {
                out.push(facts(
                    DeviceEvent::Suspended,
                    sleep,
                    None,
                    s,
                    "kernel_sleep_wake",
                ));
                out.push(facts(
                    DeviceEvent::Resumed,
                    wake,
                    None,
                    s,
                    "kernel_sleep_wake",
                ));
            }
            self.last_wake = Some(wake);
        }

        if let Some(locked) = s.locked {
            if self.locked != Some(locked) && (prev.is_some() || locked) {
                let event = if locked {
                    DeviceEvent::Locked
                } else {
                    DeviceEvent::Unlocked
                };
                out.push(facts(event, s.at, prev, s, "session_lock"));
            }
            self.locked = Some(locked);
        }

        if let Some(idle_ms) = s.idle_ms {
            let last_input = UtcMillis(s.at.0 - idle_ms.max(0));
            let idle = idle_ms >= COLLECTION_THRESHOLD_MS;
            if self.idle != Some(idle) {
                let (event, earliest) = match (idle, self.idle) {
                    // Idle began at the last input: observed.
                    (true, _) => (DeviceEvent::Idle, None),
                    // The input ending idle lies between the previous sample and the last input.
                    (false, Some(true)) => (DeviceEvent::Active, prev.filter(|p| *p < last_input)),
                    // Initial state: active since at least the last input.
                    (false, _) => (DeviceEvent::Active, None),
                };
                out.push(facts(event, last_input, earliest, s, "hid_idle_time"));
            }
            self.idle = Some(idle);
        }

        out.sort_by_key(|f| f.at);
        self.last_sample = Some(s.at);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: i64 = 1_000;

    fn sample(at: i64, idle: i64) -> DeviceSample {
        DeviceSample {
            at: UtcMillis(at * S),
            idle_ms: Some(idle * S),
            locked: Some(false),
            last_sleep_wake: Some((UtcMillis(0), UtcMillis(10 * S))),
        }
    }

    fn events(v: &[DeviceFacts]) -> Vec<(DeviceEvent, i64, &str)> {
        v.iter()
            .map(|f| (f.event, f.at.0 / S, f.basis.as_str()))
            .collect()
    }

    #[test]
    fn idle_is_placed_at_the_last_input_and_active_is_bounded() {
        let mut t = DeviceTracker::new();
        assert_eq!(
            events(&t.observe(&sample(100, 5))),
            vec![(DeviceEvent::Active, 95, "observed")]
        );
        assert!(
            t.observe(&sample(115, 20)).is_empty(),
            "below the collection threshold"
        );
        // 60 s without input: idle since the last input at 95.
        assert_eq!(
            events(&t.observe(&sample(160, 65))),
            vec![(DeviceEvent::Idle, 95, "observed")]
        );
        // Input again: the first input lies in (160, 173].
        let back = t.observe(&sample(175, 2));
        assert_eq!(events(&back), vec![(DeviceEvent::Active, 173, "bounded")]);
        assert_eq!(back[0].earliest, Some(UtcMillis(160 * S)));
        assert_eq!(
            back[0].collection_threshold_ms,
            Some(COLLECTION_THRESHOLD_MS)
        );
    }

    #[test]
    fn lock_changes_are_bounded_by_samples() {
        let mut t = DeviceTracker::new();
        t.observe(&sample(100, 1));
        let mut s = sample(115, 16);
        s.locked = Some(true);
        let v = t.observe(&s);
        assert_eq!(events(&v), vec![(DeviceEvent::Locked, 115, "bounded")]);
        assert_eq!(v[0].earliest, Some(UtcMillis(100 * S)));
    }

    #[test]
    fn a_new_kernel_wake_yields_observed_suspend_and_resume() {
        let mut t = DeviceTracker::new();
        t.observe(&sample(100, 1));
        let mut s = sample(4000, 1);
        s.last_sleep_wake = Some((UtcMillis(200 * S), UtcMillis(3990 * S)));
        let v = t.observe(&s);
        assert_eq!(
            events(&v)[..2],
            [
                (DeviceEvent::Suspended, 200, "observed"),
                (DeviceEvent::Resumed, 3990, "observed")
            ]
        );
        // The same wake is not reported twice.
        assert!(
            t.observe(&{
                let mut x = s;
                x.at = UtcMillis(4015 * S);
                x
            })
            .is_empty()
        );
    }

    #[test]
    fn a_wake_before_the_tracker_started_is_not_reported() {
        let mut t = DeviceTracker::new();
        let v = t.observe(&sample(100, 1));
        assert!(v.iter().all(|f| f.event != DeviceEvent::Resumed));
    }

    #[test]
    fn unavailable_signals_produce_nothing() {
        let mut t = DeviceTracker::new();
        let v = t.observe(&DeviceSample {
            at: UtcMillis(1),
            idle_ms: None,
            locked: None,
            last_sleep_wake: None,
        });
        assert!(v.is_empty());
    }
}
