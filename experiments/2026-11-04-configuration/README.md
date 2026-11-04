# Shared CLI and file configuration

**Completed November 4, 2026.** Configure one experiment through `conf` CLI/TOML
sources, default to full speed and an entropy seed, and retain reproducible runs.
Removed wall-time budgets and focal-agent CLI stopping; simulation rules are unchanged.

Source: the commit containing this record, tested as working changes after
`9972ed9`, using its updated lockfile (conf 0.4.5, toml 1.2.0), Rust/Cargo 1.98.1,
and `x86_64-unknown-linux-gnu`. The wgame pin remains
`58e322799a2d7448e85a8762c97801b60a8ced78`.

```sh
cargo test --locked --offline
cargo test --locked --offline --no-default-features
cargo clippy --locked --offline --all-targets -- -D warnings
cargo run --locked --bin nevo -- --config configs/debug.toml
cargo run --locked --bin nevo -- --scenario baseline --seed 1 --ticks 256
```

All 26 tests pass with and without graphics. [Configuration checks](../../tests/config_cli.rs)
compare CLI and TOML outputs, CLI overrides (including false booleans), full-range
quoted seeds, invalid/removed settings, and help without a readable file. A fresh
system-entropy seed is printed and used to replay identical output. The
[example](../../configs/debug.toml) matches the equivalent CLI run through 32 ticks.
A separate 32-tick debug run drew seed `599338905515289512` from system entropy;
its replay using the printed seed produced byte-identical output.
Baseline seed 1 at 256 ticks still has 184 meals, 4 births, 42 deaths, 38 immigrants,
and 20 living animals, matching the preceding implementation.

At the new unpaced default, both viewer smoke checks passed twelve sampled world
comparisons and pause/step controls: debug seed 1 and baseline seed 42. They used
the [isolated Xvfb/Mesa procedure](../2026-11-04-unified-loop/README.md) with a
40-second external timeout. Sampled ticks depend on scheduling; simulation state
at a fixed seed and tick does not. No adaptation claim is made.
