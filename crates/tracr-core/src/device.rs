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

/// Device-lane state (architecture §3.4 `device_interval`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneState {
    Active,
    Idle,
    Locked,
    Suspended,
    Unknown,
}

impl LaneState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Locked => "locked",
            Self::Suspended => "suspended",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInterval {
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub state: LaneState,
}

/// The device lane from sensor transitions. Outside `collecting` spans of
/// the sensor's coverage the state is `unknown`: evidence the sensor did not
/// collect is never assumed. Each run start resets what is known (the run's
/// first sample restates the device). Precedence: suspended > locked > idle > active.
pub fn device_lane(
    transitions: &[DeviceFacts],
    run_starts: &[UtcMillis],
    collecting: &[(UtcMillis, UtcMillis)],
    as_of: UtcMillis,
) -> Vec<DeviceInterval> {
    #[derive(Clone, Copy)]
    enum Change<'a> {
        Reset,
        Fact(&'a DeviceFacts),
    }
    let mut starts: Vec<i64> = run_starts.iter().map(|t| t.0).collect();
    starts.sort_unstable();
    let mut changes: Vec<(i64, u8, Change<'_>)> =
        starts.iter().map(|t| (*t, 0, Change::Reset)).collect();
    // A run's first sample places the initial state at the last input,
    // usually before the run began. It applies from the run's start: the
    // run's reset must not erase what its own sample reported.
    changes.extend(transitions.iter().map(|f| {
        let run_start = starts.iter().rev().find(|s| **s <= f.sample_at.0).copied();
        (
            run_start.map_or(f.at.0, |s| f.at.0.max(s)),
            1,
            Change::Fact(f),
        )
    }));
    changes.sort_by_key(|c| (c.0, c.1));

    // Raw state segments from the transitions alone.
    let (mut idle, mut locked, mut suspended): (Option<bool>, bool, bool) = (None, false, false);
    let state = |idle: Option<bool>, locked: bool, suspended: bool| {
        if suspended {
            LaneState::Suspended
        } else if locked {
            LaneState::Locked
        } else {
            match idle {
                Some(true) => LaneState::Idle,
                Some(false) => LaneState::Active,
                None => LaneState::Unknown,
            }
        }
    };
    let mut raw: Vec<(i64, LaneState)> = Vec::new();
    for (t, _, c) in &changes {
        match c {
            Change::Reset => (idle, locked, suspended) = (None, false, false),
            Change::Fact(f) => match f.event {
                DeviceEvent::Active => idle = Some(false),
                DeviceEvent::Idle => idle = Some(true),
                DeviceEvent::Locked => locked = true,
                DeviceEvent::Unlocked => locked = false,
                DeviceEvent::Suspended => suspended = true,
                DeviceEvent::Resumed => suspended = false,
            },
        }
        raw.push((*t, state(idle, locked, suspended)));
    }

    // Cut by coverage: outside collecting spans the lane is unknown.
    let mut bounds: Vec<i64> = raw.iter().map(|r| r.0).collect();
    for (s, e) in collecting {
        bounds.push(s.0);
        bounds.push(e.0);
    }
    bounds.push(as_of.0);
    bounds.retain(|b| *b <= as_of.0);
    bounds.sort_unstable();
    bounds.dedup();
    let raw_at = |t: i64| -> LaneState {
        raw.iter()
            .rev()
            .find(|r| r.0 <= t)
            .map_or(LaneState::Unknown, |r| r.1)
    };
    let covered = |t: i64| collecting.iter().any(|(s, e)| s.0 <= t && t < e.0);

    let mut out: Vec<DeviceInterval> = Vec::new();
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        let st = if covered(a) {
            raw_at(a)
        } else {
            LaneState::Unknown
        };
        match out.last_mut() {
            Some(last) if last.end.0 == a && last.state == st => last.end = UtcMillis(b),
            _ => out.push(DeviceInterval {
                start: UtcMillis(a),
                end: UtcMillis(b),
                state: st,
            }),
        }
    }
    out
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

    fn fact(event: DeviceEvent, at: i64) -> DeviceFacts {
        DeviceFacts {
            event,
            at: UtcMillis(at * S),
            earliest: None,
            basis: "observed".into(),
            sample_at: UtcMillis(at * S),
            collection_threshold_ms: None,
            signal: "test".into(),
        }
    }

    #[test]
    fn the_lane_follows_transitions_inside_coverage_only() {
        let t = [
            fact(DeviceEvent::Active, 100),
            fact(DeviceEvent::Idle, 200),
            fact(DeviceEvent::Locked, 260),
            fact(DeviceEvent::Unlocked, 300),
            fact(DeviceEvent::Active, 305),
            fact(DeviceEvent::Suspended, 400),
            fact(DeviceEvent::Resumed, 500),
        ];
        let cov = [
            (UtcMillis(100 * S), UtcMillis(450 * S)),
            (UtcMillis(480 * S), UtcMillis(600 * S)),
        ];
        let lane = device_lane(&t, &[UtcMillis(100 * S)], &cov, UtcMillis(600 * S));
        let v: Vec<(i64, i64, &str)> = lane
            .iter()
            .map(|d| (d.start.0 / S, d.end.0 / S, d.state.as_str()))
            .collect();
        assert_eq!(
            v,
            vec![
                (100, 200, "active"),
                (200, 260, "idle"),
                (260, 300, "locked"),
                (300, 305, "idle"),
                (305, 400, "active"),
                (400, 450, "suspended"),
                (450, 480, "unknown"),
                (480, 500, "suspended"),
                (500, 600, "active"),
            ]
        );
    }

    #[test]
    fn the_first_sample_of_a_run_is_not_erased_by_its_own_start() {
        // The run starts at 100; its first sample (at 100) reports active since 99.
        let mut f = fact(DeviceEvent::Active, 99);
        f.sample_at = UtcMillis(100 * S);
        let cov = [(UtcMillis(100 * S), UtcMillis(200 * S))];
        let lane = device_lane(&[f], &[UtcMillis(100 * S)], &cov, UtcMillis(200 * S));
        assert_eq!(lane.last().unwrap().state, LaneState::Active);
        assert_eq!(lane.last().unwrap().start, UtcMillis(100 * S));
    }

    #[test]
    fn a_new_run_forgets_the_previous_lock() {
        let t = [
            fact(DeviceEvent::Active, 0),
            fact(DeviceEvent::Locked, 10),
            fact(DeviceEvent::Active, 100),
        ];
        let cov = [
            (UtcMillis(0), UtcMillis(20 * S)),
            (UtcMillis(100 * S), UtcMillis(200 * S)),
        ];
        let lane = device_lane(
            &t,
            &[UtcMillis(0), UtcMillis(100 * S)],
            &cov,
            UtcMillis(200 * S),
        );
        assert_eq!(lane.last().unwrap().state, LaneState::Active);
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
