# tz — blazing-fast timezone CLI

Convert times across cities and browse an interactive world clock, with correct
DST handling for every scenario. Native Rust, ~1.7 ms startup measured over 50 runs on Linux x86-64, single 1.35 MiB
self-contained binary (the IANA tz database is compiled in).

## Usage

```
tz                          interactive world clock (your saved zones)
tz london toronto           current time in each
tz 4pm london toronto       4pm London, shown in each
tz 4pm tomorrow london nyc  natural-language time + date
tz 2am london madrid tokyo  one anchor, many targets
tz -i london sydney nyc     open the interactive view seeded with these
tz --save london nyc tokyo  set your default zones
tz --recent                 list recently/frequently checked cities
```

The interactive dashboard (bare `tz`) shows your saved zones, then a separator
and the cities you've **recently and frequently checked** (ranked by frecency,
with a `×N` count). Every city you look up is remembered automatically.

The **first city is the anchor**: when you give a time, it's interpreted in that
zone and converted to all the others. With no time, it shows the current moment
everywhere.

**Time:** `4pm` `4:30pm` `16:00` `1600` `9am` `noon` `midnight`
**Date:** `today` `tomorrow` `friday` `next monday` `jul 1` `2026-07-01` `7/1`
**City:** `london` `nyc` `tokyo` `hong kong` `new york` `pst` `utc` `Europe/Madrid`
`local` — multi-word names group automatically, plus fuzzy matching against the
full IANA database (typos like `torrento` → Toronto resolve).

### Interactive keys

```
↑/↓  k/j      select city      ←/→  h/l    ± 1 hour
[ / ]         ± 15 minutes
Ctrl+←/→      ± 1 day          Ctrl+↑/↓     ± 1 week
PgUp / PgDn   ± 1 month
t / n / 0     jump to now      q / Esc      quit
```
Weekends (Sat/Sun) are gently tinted in the calendar and zone rows.

## Correctness

Built on [jiff](https://docs.rs/jiff), which handles the hard cases:

- **DST spring-forward gaps** — `tz 2:30am 2026-03-08 nyc` (a time that doesn't
  exist) is adjusted forward with a warning.
- **DST fall-back folds** — `tz 1:30am 2026-11-01 nyc` (a time that happens
  twice) resolves to the first occurrence with a warning.
- **Sub-hour offsets** — India (+5:30), Nepal (+5:45), Chatham (+12:45).
- **Historical rules** — past/future dates use the correct rules for that date.

## Install

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
git clone https://github.com/kidsil/tz-cli.git
cd tz-cli
cargo install --path .          # installs `tz` into ~/.cargo/bin
```

Or build without installing:

```sh
cargo build --release           # -> target/release/tz
cargo test
```

The IANA timezone database is compiled into the binary, so there is nothing to
install alongside it and no network access at runtime.

## Licence

Dual licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT licence ([LICENSE-MIT](LICENSE-MIT))

at your option. Contributions are accepted under the same dual licence.

## Config

Two files under `~/.config/tz` (or `$XDG_CONFIG_HOME/tz`):

- `zones` — default zones for bare `tz`, one city per line, `#` comments
  allowed. Created on first `tz --save`.
- `history` — the cities you look up, with counts, so the interactive view can
  offer the ones you actually use. Written on every lookup, created on first
  ordinary use. It stays on your machine and is sent nowhere. Delete it to
  clear it: `rm ~/.config/tz/history`.

## Layout

```
src/main.rs     arg parsing + dispatch
src/parse.rs    natural-language time & date parser
src/cities.rs   city/abbreviation → IANA zone (curated aliases + fuzzy)
src/render.rs   one-shot aligned/colored output + DST notes
src/tui.rs      interactive navigable world clock + calendar (ratatui)
src/config.rs   saved default zones
```
