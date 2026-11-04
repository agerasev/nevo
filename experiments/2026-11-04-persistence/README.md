# Checkpoint continuation

**Completed November 4, 2026.** Check whether writing and restoring a complete
CBOR checkpoint preserves the foraging trajectory, counters, and viewer controls.
Simulation rules and initialization are unchanged; save policy is the changed factor.

Source: the commit containing this record, tested as working changes after
`949447c`, with its lockfile, Rust/Cargo 1.98.1, and `x86_64-unknown-linux-gnu`.
CBOR uses ciborium 0.2.2; no safetensors. The wgame pin remains
`58e322799a2d7448e85a8762c97801b60a8ced78`.

```sh
cargo test --locked --offline
cargo test --locked --offline --no-default-features
cargo clippy --locked --offline --all-targets -- -D warnings
cargo clippy --locked --offline --no-default-features --all-targets -- -D warnings
```

All 42 tests pass with and without viewer features. The
[snapshot tests](../../src/checkpoint.rs) save debug and baseline seeds 1/42 at
tick 73 and compare every report and full state through tick 256, crossing death
and immigration. They also cover founder templates, recurrent memory, negative
energy awaiting culling, and rejection of invalid state/model identifiers.

The [CLI tests](../../tests/persistence_cli.rs) compare baseline seed 1 through
256 uninterrupted ticks against 128 ticks followed by a new process with only
the run-directory argument. Full snapshots and cumulative counts match, as do
64 further ticks. Other checks cover exit-only saves, zero-tick runs, retention
(25 ticks, period 10, keep 2 leaves ticks 20/25), full-range seeds, missing-config
recovery, ignored partial writes, corrupt-checkpoint rejection, and writer locking.
Injected encoding/publication failures preserve the previous checkpoint. Ctrl-C
saves and releases the lock; separate paused SIGINT/SIGTERM checks saved tick zero.

Viewer checks used the [isolated Xvfb/Mesa procedure](../2026-11-04-unified-loop/README.md),
with a 40-second external timeout. Commands below use fresh temporary directories:

```sh
target/debug/nevo "$BASELINE" --seed 42 --ticks 64 \
  --checkpoint-every 16 --keep-checkpoints 3
target/debug/nevo "$BASELINE" --viewer --smoke --ticks 1000000 \
  --checkpoint-every 100 --keep-checkpoints 2
target/debug/nevo "$DEBUG" --viewer --smoke --scenario debug --seed 1 \
  --hz 25 --checkpoint-every 1 --keep-checkpoints 2
```

Both passed twelve displayed-world comparisons and pause/step controls. The
resumed baseline ended at tick 502 (349 meals, 2 births, 79 deaths, 77 immigrants);
the fresh debug run ended at tick 6 (1 meal, 1 birth). A separate Rust audit loaded
their final CBOR files and compared complete snapshots and counters against
uninterrupted `Run::execute` references at those ticks: exact matches. Baseline
retained ticks 500/502; debug retained 5/6. Sampled ticks depend on scheduling.
Temporary run artifacts and audit source were under `/tmp/nevo-persistence.H6Z8u3/`.

This establishes continuation within the tested build/platform, not evolutionary
adaptation or cross-version compatibility. [Persistence contract](../../docs/persistence.md)
