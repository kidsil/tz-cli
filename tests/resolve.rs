//! Resolution regression tests: country names, common typos, multi-word names,
//! and the negative cases (junk must fail, a prefix-less word must not match).
//! Checks the displayed city name (season-independent, unlike offsets).

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tz");

fn run(args: &[&str]) -> (bool, String) {
    // Isolate config/history so tests never touch the real ~/.config/tz.
    let cfg = format!("{}/resolve-cfg", env!("CARGO_TARGET_TMPDIR"));
    let out = Command::new(BIN)
        .args(args)
        .env("XDG_CONFIG_HOME", &cfg)
        .output()
        .expect("failed to run tz");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn resolves_country_names() {
    // (args, expected city in output)
    let cases: &[(&[&str], &str)] = &[
        (&["poland"], "Warsaw"),
        (&["japan"], "Tokyo"),
        (&["brazil"], "Sao Paulo"),
        (&["canada"], "Toronto"),
        (&["india"], "Kolkata"),
        (&["australia"], "Sydney"),
        (&["iran"], "Tehran"),
        (&["vietnam"], "Ho Chi Minh"),
        // multi-word country names (grouped from separate argv tokens)
        (&["south", "korea"], "Seoul"),
        (&["united", "kingdom"], "London"),
        (&["new", "zealand"], "Auckland"),
    ];
    let mut fails = Vec::new();
    for (args, want) in cases {
        let (ok, out) = run(args);
        if !ok || !out.contains(want) {
            fails.push(format!("{args:?} -> expected '{want}', got: {}", out.trim()));
        }
    }
    assert!(fails.is_empty(), "country resolution failures:\n{}", fails.join("\n"));
}

#[test]
fn tolerates_real_typos() {
    let cases: &[(&[&str], &str)] = &[
        (&["warsha"], "Warsaw"),
        (&["torrento"], "Toronto"),
        (&["berln"], "Berlin"),
        (&["tokio"], "Tokyo"),
    ];
    let mut fails = Vec::new();
    for (args, want) in cases {
        let (ok, out) = run(args);
        if !ok || !out.contains(want) {
            fails.push(format!("{args:?} -> expected '{want}', got: {}", out.trim()));
        }
    }
    assert!(fails.is_empty(), "typo resolution failures:\n{}", fails.join("\n"));
}

#[test]
fn rejects_junk_and_false_matches() {
    // Pure junk must fail outright.
    let (ok, _) = run(&["xyzzy"]);
    assert!(!ok, "'xyzzy' should not resolve");

    let (ok, _) = run(&["qwertzuiop"]);
    assert!(!ok, "'qwertzuiop' should not resolve");

    // A short word sharing no prefix must not edit-distance its way into a
    // far-off zone: "adis" is one edit from "Davis" and must never reach it.
    let (_ok, out) = run(&["adis"]);
    assert!(!out.contains("Davis"), "'adis' must never resolve to Davis: {}", out.trim());
}
