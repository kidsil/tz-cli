//! DST backtest: drives the built `tz` binary end-to-end and asserts UTC
//! offsets across zones with DIFFERENT DST rules, different transition dates,
//! and rules that CHANGED between years (US 2007 reform, Brazil/Iran abolition),
//! plus spring-forward gaps and fall-back folds.
//!
//! Run with `cargo test`. Uses the freshly built binary via CARGO_BIN_EXE_tz.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tz");

fn run(args: &[&str]) -> String {
    // Isolate config/history so tests never touch the real ~/.config/tz.
    let cfg = format!("{}/dst-cfg", env!("CARGO_TARGET_TMPDIR"));
    let out = Command::new(BIN)
        .args(args)
        .env("XDG_CONFIG_HOME", &cfg)
        .output()
        .expect("failed to run tz");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

/// Extract the first "UTC+H" / "UTC-H" / "UTC+H:MM" token from the output.
fn first_offset(s: &str) -> Option<String> {
    let i = s.find("UTC")?;
    let b = s.as_bytes();
    let mut j = i + 3;
    if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
        j += 1;
    } else {
        return None;
    }
    while j < b.len() && (b[j].is_ascii_digit() || b[j] == b':') {
        j += 1;
    }
    Some(s[i..j].to_string())
}

fn has_warning(s: &str) -> bool {
    s.contains('⚠')
}

struct Case {
    label: &'static str,
    args: &'static [&'static str],
    offset: &'static str,
    warn: bool,
}

const C: fn(&'static str, &'static [&'static str], &'static str, bool) -> Case =
    |label, args, offset, warn| Case { label, args, offset, warn };

fn cases() -> Vec<Case> {
    vec![
        // --- US rule CHANGE between years (2007 Energy Policy Act) ---
        C("US mid-Mar 2006 (old rule: not yet DST)", &["noon", "2006-03-15", "nyc"], "UTC-5", false),
        C("US mid-Mar 2007 (new rule: already DST)", &["noon", "2007-03-15", "nyc"], "UTC-4", false),
        C("US mid-Apr 2006 (old rule: DST since Apr 2)", &["noon", "2006-04-15", "nyc"], "UTC-4", false),
        C("US winter 2025", &["noon", "2025-01-15", "nyc"], "UTC-5", false),
        C("US summer 2025", &["noon", "2025-07-15", "nyc"], "UTC-4", false),
        // --- EU: last Sunday of March/October (different entry point than US) ---
        C("London day BEFORE EU switch (2025-03-29)", &["noon", "2025-03-29", "london"], "UTC+0", false),
        C("London day AFTER EU switch (2025-03-31)", &["noon", "2025-03-31", "london"], "UTC+1", false),
        C("London winter 2025", &["noon", "2025-01-15", "london"], "UTC+0", false),
        C("London summer 2025", &["noon", "2025-07-15", "london"], "UTC+1", false),
        C("Berlin winter 2025", &["noon", "2025-01-15", "berlin"], "UTC+1", false),
        C("Berlin summer 2025", &["noon", "2025-07-15", "berlin"], "UTC+2", false),
        // --- Southern hemisphere: seasons REVERSED ---
        C("Sydney our-summer = their winter", &["noon", "2025-07-15", "sydney"], "UTC+10", false),
        C("Sydney our-winter = their summer (DST)", &["noon", "2025-01-15", "sydney"], "UTC+11", false),
        // --- Lord Howe Island: unique 30-MINUTE DST shift ---
        C("Lord Howe summer (DST, +11)", &["noon", "2025-01-15", "Australia/Lord_Howe"], "UTC+11", false),
        C("Lord Howe winter (std, +10:30)", &["noon", "2025-07-15", "Australia/Lord_Howe"], "UTC+10:30", false),
        // --- Brazil: DST ABOLISHED after 2019 (rule change between years) ---
        C("Sao Paulo 2018 (DST era, -2)", &["noon", "2018-01-15", "sao", "paulo"], "UTC-2", false),
        C("Sao Paulo 2022 (no DST, -3)", &["noon", "2022-01-15", "sao", "paulo"], "UTC-3", false),
        // --- Iran: DST ABOLISHED 2022 + half-hour offset ---
        C("Tehran 2021 (DST era, +4:30)", &["noon", "2021-07-15", "Asia/Tehran"], "UTC+4:30", false),
        C("Tehran 2023 (no DST, +3:30)", &["noon", "2023-07-15", "Asia/Tehran"], "UTC+3:30", false),
        // --- India: NO DST at all (sanity) ---
        C("Delhi winter (no DST)", &["noon", "2025-01-15", "delhi"], "UTC+5:30", false),
        C("Delhi summer (no DST)", &["noon", "2025-07-15", "delhi"], "UTC+5:30", false),
        // --- Chatham Islands: +12:45 / +13:45, NZ DST dates ---
        C("Chatham summer (DST, +13:45)", &["noon", "2025-01-15", "Pacific/Chatham"], "UTC+13:45", false),
        C("Chatham winter (+12:45)", &["noon", "2025-07-15", "Pacific/Chatham"], "UTC+12:45", false),
        // --- Israel: own DST rule ---
        C("Jerusalem summer (IDT, +3)", &["noon", "2025-07-15", "jerusalem"], "UTC+3", false),
        C("Jerusalem winter (IST, +2)", &["noon", "2025-01-15", "jerusalem"], "UTC+2", false),
        // --- DST GAPS (spring-forward, time doesn't exist) -> warn + adjusted ---
        C("US gap 2:30am 2026-03-08 -> 3:30 EDT", &["2:30am", "2026-03-08", "nyc"], "UTC-4", true),
        C("London gap 1:30am 2025-03-30 -> 2:30 BST", &["1:30am", "2025-03-30", "london"], "UTC+1", true),
        // --- DST FOLD (fall-back, time happens twice) -> warn + first ---
        C("US fold 1:30am 2025-11-02 (first=EDT)", &["1:30am", "2025-11-02", "nyc"], "UTC-4", true),
    ]
}

#[test]
fn dst_backtest() {
    let mut failures = Vec::new();
    for c in cases() {
        let out = run(c.args);
        let got = first_offset(&out).unwrap_or_else(|| "(none)".to_string());
        let warn = has_warning(&out);
        if got != c.offset || warn != c.warn {
            failures.push(format!(
                "{}: args={:?} expected {}/warn={}, got {}/warn={}",
                c.label, c.args, c.offset, c.warn, got, warn
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "DST backtest had {} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
