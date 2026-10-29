//! Cost alerts: thresholds, doublings, nothing on the first look, periods.

use tracr_app::costwatch::{CostAlertPrefs, CostWatcher, Kind, Measures};

fn prefs() -> CostAlertPrefs {
    CostAlertPrefs {
        enabled: true,
        session: 25,
        window: 15,
        window_minutes: 30,
        day: 60,
        month: 0,
    }
}

fn m(session: f64, window: f64, day: f64) -> Measures {
    Measures {
        sessions: vec![("s1".into(), Some("harbor".into()), session)],
        window,
        day,
        month: 0.0,
    }
}

#[test]
fn an_alert_fires_when_a_figure_crosses_and_at_each_doubling_not_in_between() {
    let mut w = CostWatcher::default();
    assert!(
        w.changes(&m(5.0, 1.0, 10.0), &prefs(), "2026-10-27")
            .is_empty(),
        "first look is silent"
    );
    assert!(
        w.changes(&m(24.0, 5.0, 30.0), &prefs(), "2026-10-27")
            .is_empty(),
        "below every threshold"
    );
    let a = w.changes(&m(26.0, 5.0, 30.0), &prefs(), "2026-10-27");
    assert_eq!(a.len(), 1);
    assert!(matches!(&a[0].kind, Kind::Session { label } if label.as_deref() == Some("harbor")));
    assert_eq!(a[0].crossed, 25.0);
    assert!(
        w.changes(&m(40.0, 5.0, 30.0), &prefs(), "2026-10-27")
            .is_empty(),
        "between 25 and 50"
    );
    let b = w.changes(&m(51.0, 5.0, 30.0), &prefs(), "2026-10-27");
    assert_eq!((b.len(), b[0].crossed), (1, 50.0), "the doubling");
    assert!(
        w.changes(&m(60.0, 5.0, 30.0), &prefs(), "2026-10-27")
            .is_empty()
    );
    let c = w.changes(&m(101.0, 5.0, 30.0), &prefs(), "2026-10-27");
    assert_eq!(c[0].crossed, 100.0);
}

#[test]
fn window_and_day_alerts_and_a_new_day_starts_again() {
    let mut w = CostWatcher::default();
    w.changes(&m(1.0, 1.0, 10.0), &prefs(), "2026-10-27");
    let a = w.changes(&m(1.0, 16.0, 61.0), &prefs(), "2026-10-27");
    assert_eq!(a.len(), 2);
    assert!(
        a.iter()
            .any(|x| matches!(x.kind, Kind::Window { minutes: 30 }))
    );
    assert!(a.iter().any(|x| x.kind == Kind::Day));
    // Next day: the day figure is a new one; it alerts again when it crosses again.
    assert!(
        w.changes(&m(1.0, 1.0, 5.0), &prefs(), "2026-10-28")
            .is_empty()
    );
    assert_eq!(
        w.changes(&m(1.0, 1.0, 62.0), &prefs(), "2026-10-28").len(),
        1
    );
}

#[test]
fn switched_off_or_zero_thresholds_never_alert() {
    let mut w = CostWatcher::default();
    let off = CostAlertPrefs {
        enabled: false,
        ..prefs()
    };
    w.changes(&m(1.0, 1.0, 1.0), &off, "d");
    assert!(w.changes(&m(999.0, 999.0, 999.0), &off, "d").is_empty());
    let zero = CostAlertPrefs {
        enabled: true,
        session: 0,
        window: 0,
        day: 0,
        month: 0,
        window_minutes: 30,
    };
    w.changes(&m(1.0, 1.0, 1.0), &zero, "d");
    assert!(w.changes(&m(999.0, 999.0, 999.0), &zero, "d").is_empty());
}
