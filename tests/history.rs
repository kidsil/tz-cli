//! History tests: checking cities records them, and `--recent` lists them
//! ranked by frecency (more-checked ranks higher). Isolated via a temporary
//! XDG_CONFIG_HOME so the real user config is never touched.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_tz");
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

fn run_in(cfg: &PathBuf, args: &[&str]) -> (bool, String) {
    let out = Command::new(BIN)
        .args(args)
        .env("XDG_CONFIG_HOME", cfg)
        .output()
        .expect("failed to run tz");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn records_checks_and_ranks_recent() {
    let cfg = PathBuf::from(TMP).join("history_test_cfg");
    let _ = std::fs::remove_dir_all(&cfg);
    std::fs::create_dir_all(&cfg).expect("mk cfg");

    // Empty history first.
    let (ok, out) = run_in(&cfg, &["--recent"]);
    assert!(ok && out.contains("No history yet"), "expected empty history: {out}");

    // Check some cities; Warsaw twice so it should rank highest.
    run_in(&cfg, &["toronto"]);
    run_in(&cfg, &["japan"]);
    run_in(&cfg, &["poland"]);
    run_in(&cfg, &["poland"]);

    let (ok, out) = run_in(&cfg, &["--recent"]);
    assert!(ok, "--recent should succeed");
    assert!(out.contains("Warsaw"), "recent should include Warsaw:\n{out}");
    assert!(out.contains("Tokyo"), "recent should include Tokyo:\n{out}");
    assert!(out.contains("Toronto"), "recent should include Toronto:\n{out}");

    // Warsaw (checked x2) must rank above Tokyo (x1).
    let wi = out.find("Warsaw").unwrap();
    let ti = out.find("Tokyo").unwrap();
    assert!(wi < ti, "Warsaw (×2) should rank above Tokyo (×1):\n{out}");

    // The history file is written under the isolated config dir.
    assert!(cfg.join("tz").join("history").exists(), "history file should exist");
}
