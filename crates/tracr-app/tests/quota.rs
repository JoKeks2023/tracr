//! Quota judgement (after ClaudeBar's documented behaviour): levels, pace,
//! forecast, the menu bar text and "only when it gets worse".

use tracr_app::limits::LimitWindow;
use tracr_app::quota::{
    Level, Pace, QuotaPrefs, Watcher, forecast, level, menu_bar_text, tightest,
};

const OBSERVED: i64 = 1_000_000_000_000;

/// A 5-hour window `elapsed` of the way through, `used` percent used.
fn win(used: f64, elapsed: f64) -> LimitWindow {
    let length = 300 * 60_000_i64;
    let start = OBSERVED - (elapsed * length as f64) as i64;
    LimitWindow {
        provider: "claude_code".into(),
        window: "five_hour".into(),
        window_minutes: Some(300),
        used_percent: used,
        resets_at: Some(start + length),
        observed_at: OBSERVED,
        plan: None,
        expired: false,
    }
}

#[test]
fn half_used_at_half_time_is_on_track_and_lasts() {
    let f = forecast(&win(52.0, 0.5)).unwrap();
    assert_eq!(f.pace, Pace::OnTrack);
    assert_eq!(f.runs_out_at, None);
    assert_eq!(level(&win(52.0, 0.5), true), Level::Ok);
    assert_eq!(
        level(&win(52.0, 0.5), false),
        Level::Warn,
        "level alone: under 50 % left"
    );
}

#[test]
fn running_hot_says_when_it_runs_out_before_the_reset() {
    let w = win(60.0, 0.5);
    let f = forecast(&w).unwrap();
    assert_eq!(f.pace, Pace::Hot);
    let out = f.runs_out_at.unwrap();
    assert_eq!((out - OBSERVED) / 60_000, 100, "1h 40m from the statement");
    assert_eq!(
        (w.resets_at.unwrap() - out) / 60_000,
        50,
        "50m before the reset"
    );
    assert_eq!(level(&w, true), Level::Warn);
}

#[test]
fn room_to_spare_is_healthy_even_below_half_but_critical_stays_absolute() {
    assert_eq!(forecast(&win(40.0, 0.9)).unwrap().pace, Pace::Room);
    assert_eq!(level(&win(40.0, 0.9), true), Level::Ok);
    assert_eq!(level(&win(85.0, 0.95), true), Level::Crit);
    assert_eq!(level(&win(100.0, 0.5), true), Level::Depleted);
}

#[test]
fn no_pace_in_the_first_minutes_or_without_a_window() {
    assert!(forecast(&win(10.0, 0.01)).is_none());
    let mut w = win(10.0, 0.5);
    w.window_minutes = None;
    assert!(forecast(&w).is_none());
    let mut e = win(60.0, 0.5);
    e.expired = true;
    assert!(forecast(&e).is_none());
    assert_eq!(level(&e, true), Level::Ok, "an expired figure is history");
}

#[test]
fn menu_bar_text_shows_the_tightest_window_with_its_countdown() {
    let a = win(62.0, 0.5); // resets in 2h 30m
    let mut b = win(26.0, 0.5);
    b.window = "weekly".into();
    b.window_minutes = Some(10_080);
    let windows = [a.clone(), b];
    assert_eq!(tightest(&windows).unwrap().used_percent, 62.0);
    let used = QuotaPrefs::default();
    assert_eq!(
        menu_bar_text(&windows, &used, OBSERVED).unwrap(),
        "62% · 2h30m"
    );
    let left = QuotaPrefs {
        remaining: true,
        ..used
    };
    assert_eq!(
        menu_bar_text(&windows, &left, OBSERVED).unwrap(),
        "38% · 2h30m"
    );
    let crit = [win(95.0, 0.5)];
    assert!(
        menu_bar_text(&crit, &used, OBSERVED)
            .unwrap()
            .ends_with('!')
    );
    assert!(menu_bar_text(&[], &used, OBSERVED).is_none());
}

#[test]
fn only_a_change_for_the_worse_is_reported_and_the_first_look_is_silent() {
    let mut w = Watcher::default();
    assert!(w.changes(&[win(10.0, 0.5)], false).is_empty(), "first look");
    assert!(
        w.changes(&[win(12.0, 0.5)], false).is_empty(),
        "still healthy"
    );
    let up = w.changes(&[win(55.0, 0.5)], false);
    assert_eq!(up.len(), 1);
    assert_eq!(up[0].level, Level::Warn);
    assert!(w.changes(&[win(58.0, 0.5)], false).is_empty(), "same level");
    let crit = w.changes(&[win(85.0, 0.5)], false);
    assert_eq!(crit[0].level, Level::Crit);
    assert!(
        w.changes(&[win(40.0, 0.5)], false).is_empty(),
        "recovering is silent"
    );
    let again = w.changes(&[win(85.0, 0.5)], false);
    assert_eq!(again.len(), 1, "worse again after recovering");
}

#[test]
fn prefs_round_trip_and_unknown_files_fall_back_to_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    assert_eq!(
        tracr_app::quota::load_prefs(dir.path()),
        QuotaPrefs::default()
    );
    let p = QuotaPrefs {
        remaining: true,
        pace_aware: false,
        menu_bar: false,
        notify: false,
        rings: true,
    };
    tracr_app::quota::save_prefs(dir.path(), &p).unwrap();
    assert_eq!(tracr_app::quota::load_prefs(dir.path()), p);
    std::fs::write(dir.path().join("quota_prefs.json"), "not json").unwrap();
    assert_eq!(
        tracr_app::quota::load_prefs(dir.path()),
        QuotaPrefs::default()
    );
}

#[test]
fn the_first_look_is_silent_for_every_window_not_only_the_first() {
    let mut w = Watcher::default();
    let mut weekly = win(90.0, 0.5);
    weekly.window = "weekly".into();
    weekly.window_minutes = Some(10_080);
    assert!(w.changes(&[win(10.0, 0.5), weekly], false).is_empty());
}
