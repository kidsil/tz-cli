//! tz — blazing-fast timezone CLI.
//!
//!   tz                          interactive world clock (saved zones)
//!   tz london toronto           current time in each
//!   tz 4pm london toronto       4pm London, shown in each
//!   tz 4pm tomorrow london nyc  natural-language time + date
//!   tz 2am london madrid tokyo  one anchor, many targets
//!   tz -i london sydney nyc     open the interactive view seeded with these
//!   tz --save london nyc tokyo  set your default zones

mod cities;
mod config;
mod history;
mod parse;
mod render;
mod tui;

use std::io::IsTerminal;
use std::process::ExitCode;

use cities::Resolved;
use parse::extract_when;

const HELP: &str = "\
tz — blazing-fast timezone CLI

USAGE:
    tz [TIME] [DATE] [CITY...]      convert/show times (print and exit)
    tz                             interactive world clock (your saved zones)
    tz -i [CITY...]                interactive view, optionally seeded
    tz --save [CITY...]            save your default zones

EXAMPLES:
    tz london toronto
    tz 4pm london toronto
    tz 4pm tomorrow london toronto
    tz 2am london madrid tokyo
    tz next monday 9am nyc london

TIME    4pm  4:30pm  16:00  1600  9am  noon  midnight
DATE    today  tomorrow  friday  next monday  jul 1  2026-07-01  7/1
CITY    london  nyc  tokyo  pst  utc  Europe/Madrid  local

OPTIONS:
    -i, --interactive   open the navigable TUI
        --save          persist the given cities as defaults
        --recent        list your recently/frequently checked cities
    -24, --24           24-hour clock (default)   --12   12-hour clock
    -h, --help          this help            -V, --version

INTERACTIVE KEYS:
    ↑/↓ select city   ←/→ ±1h   [/] ±15m   Ctrl+←/→ ±1d   Ctrl+↑/↓ ±1wk   PgUp/Dn ±1mo   t now   q quit
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tz: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut interactive = false;
    let mut save = false;
    let mut list_recent = false;
    let mut use_24h = true; // 24-hour clock by default; `--12` reverts
    let mut tokens: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("tz {}", env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
            "-i" | "--interactive" => interactive = true,
            "--save" => save = true,
            "--recent" | "--history" => list_recent = true,
            "-24" | "--24" | "--24h" => use_24h = true,
            "-12" | "--12" | "--12h" => use_24h = false,
            // An unknown flag must not fall through to the city list. City
            // normalisation strips '-', so "-v" resolved to Vaduz and "--paris"
            // to Paris, each answering confidently about the wrong place.
            other if other.starts_with('-') && other.len() > 1 => {
                eprintln!("tz: unknown option '{other}'. Try `tz --help`.");
                return Ok(ExitCode::FAILURE);
            }
            other => tokens.push(other.to_string()),
        }
    }

    let (when, city_tokens) = extract_when(&tokens);

    // --save: validate + persist, don't display.
    if save {
        if city_tokens.is_empty() {
            return Err("nothing to save — give me some cities, e.g. tz --save london nyc tokyo".into());
        }
        let resolved = cities::resolve_sequence(&city_tokens).map_err(unresolved_err)?;
        // Persist one (deduped) city phrase per line, e.g. "hong kong".
        let mut phrases: Vec<String> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (phrase, r) in resolved {
            if !seen.contains(&r.iana) {
                seen.push(r.iana);
                phrases.push(phrase);
            }
        }
        let path = config::save(&phrases)?;
        println!("Saved {} default zone(s) to {}", phrases.len(), path.display());
        return Ok(ExitCode::SUCCESS);
    }

    // --recent: list recently/frequently checked cities at the current time.
    if list_recent {
        let zones: Vec<Resolved> = history::recent(&[], 10).into_iter().map(|(z, _)| z).collect();
        if zones.is_empty() {
            println!("No history yet — check some cities (e.g. `tz 5pm poland toronto`) and they'll show up here.");
            return Ok(ExitCode::SUCCESS);
        }
        render::render(&zones, &parse::When::default(), &render::Options { use_24h })?;
        return Ok(ExitCode::SUCCESS);
    }

    // Determine the zone list.
    let zones: Vec<Resolved> = if city_tokens.is_empty() {
        default_zones()?
    } else {
        resolve_cities(&city_tokens)?
    };

    // Remember the cities the user explicitly asked about (not bare-`tz` defaults).
    if !city_tokens.is_empty() {
        history::record(&zones);
    }

    // Interactive when asked, or when bare `tz` with no time/date - but only
    // with a terminal to draw on. Piped, redirected, in CI or in a container
    // without a TTY, the TUI backend aborts the process, so bare `tz` has to
    // fall through to the plain table instead.
    // Both ends matter. Gating on stdout alone still entered the TUI when
    // stdout was a terminal but stdin was EOF or a pipe, and the event loop
    // then spun forever holding the alternate screen.
    let tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if (interactive || (city_tokens.is_empty() && when.is_empty())) && tty {
        // Only true on the path that actually discards it.
        if interactive && !when.is_empty() {
            eprintln!("tz: -i is live, so the given time/date is ignored");
        }
        let exclude: Vec<String> = zones.iter().map(|z| z.iana.clone()).collect();
        let recent = history::recent(&exclude, 8);
        tui::run(zones, recent, use_24h)?;
        return Ok(ExitCode::SUCCESS);
    }
    if interactive && !tty {
        eprintln!("tz: no terminal available, printing the table instead of -i");
    }

    render::render(&zones, &when, &render::Options { use_24h })?;
    Ok(ExitCode::SUCCESS)
}

/// True for a token shaped like a date the user typed, so a token that failed
/// date parsing is reported as a bad date rather than as a bad city.
fn looks_like_a_date(t: &str) -> bool {
    let digits = t.chars().filter(char::is_ascii_digit).count();
    (t.contains('/') || t.contains('-')) && digits >= 3
}

/// Resolve CLI city words into zones, grouping multi-word names ("hong kong")
/// and collapsing duplicate zones (same IANA id) so `tz london uk` shows once.
fn resolve_cities(tokens: &[String]) -> Result<Vec<Resolved>, Box<dyn std::error::Error>> {
    let resolved = cities::resolve_sequence(tokens).map_err(unresolved_err)?;
    let mut out: Vec<Resolved> = Vec::new();
    for (_phrase, r) in resolved {
        if !out.iter().any(|e| e.iana == r.iana) {
            out.push(r);
        }
    }
    Ok(out)
}

fn unresolved_err(bad: Vec<String>) -> Box<dyn std::error::Error> {
    // Name the real problem. "2/29/2027" is not a city the user misspelled.
    if let Some(t) = bad.iter().find(|t| looks_like_a_date(t)) {
        return format!("no such date: {t}").into();
    }
    format!(
        "couldn't resolve: {}. Try a city name, IANA zone (Europe/London), or abbreviation (pst, cet).",
        bad.join(", ")
    )
    .into()
}

/// Zones for bare `tz`: saved config, else a sensible world-clock default.
fn default_zones() -> Result<Vec<Resolved>, Box<dyn std::error::Error>> {
    let saved = config::load();
    let tokens = if saved.is_empty() {
        vec![
            "local".to_string(),
            "new york".to_string(),
            "london".to_string(),
            "tokyo".to_string(),
        ]
    } else {
        saved
    };
    // Resolve, skipping any that no longer resolve rather than failing hard,
    // and collapsing duplicate zones.
    let mut zones: Vec<Resolved> = Vec::new();
    for t in &tokens {
        if let Some(r) = cities::resolve(t) {
            if !zones.iter().any(|e| e.iana == r.iana) {
                zones.push(r);
            }
        }
    }
    if zones.is_empty() {
        return Err("no usable default zones".into());
    }
    Ok(zones)
}
