# Run persistence

Each invocation uses a run directory. Supply its path as the positional argument;
omit it to create a unique directory under `runs/`. The chosen path is printed to
stderr. Keep screenshots and other run outputs alongside checkpoints in that
directory, for example under `screenshots/`. All generated run data stays out of Git.

```sh
# Create a run, saving every 100 ticks and keeping the latest three checkpoints.
cargo run --locked -- runs/baseline --seed 1 --ticks 256 \
  --checkpoint-every 100 --keep-checkpoints 3

# Resume from its newest checkpoint, loading config.toml automatically.
cargo run --locked -- runs/baseline

# Continue for 64 ticks, saving only on exit, retaining one checkpoint.
cargo run --locked -- runs/baseline --ticks 64 \
  --checkpoint-every 0 --keep-checkpoints 1
```

`config.toml` records the last effective configuration, including the resolved
entropy seed. Ordered `--config PATH` files override individual settings, then CLI
values override the files. See [configuration precedence](configuration.md).
If `config.toml` is missing, the checkpoint's embedded configuration is recovered.
Resume rejects changed seeds or ecosystem settings. Runtime settings such as pacing,
viewer attachment, tracing, and save policy can change. A tick budget always applies to
the current invocation: a saved budget of 256 means another 256 ticks on bare
resume. Remove `ticks` from `config.toml` for an unlimited continuation.

| CLI | TOML | Default | Meaning |
| --- | --- | --- | --- |
| `--checkpoint-every N` | `checkpoint_every` | `10000` | Save at absolute world ticks divisible by N; zero disables periodic saves. |
| `--keep-checkpoints N` | `keep_checkpoints` | `3` | Retain the newest N checkpoints; must be positive. |

The worker also saves when its budget ends, the viewer closes, or a stop signal
arrives (Ctrl-C; also SIGTERM/SIGHUP on Unix). Saving the same tick replaces that
checkpoint. A zero-tick invocation still saves. There is no automatic initial
checkpoint; if interrupted before its first save, a directory with only a config
reinitializes from that saved seed.

## Layout and consistency

```text
runs/baseline/
  config.toml
  .lock
  checkpoints/
    00000000000000000100.cbor
    00000000000000000200.cbor
    00000000000000000256.cbor
```

Each checkpoint is one CBOR file containing the effective configuration, cumulative
event counts, model identifiers, and the directly serialized
[Universe](../src/universe/mod.rs): step and complete experiment model. The ecosystem
owns ordered plants and animals (each with body and mind), both RNG states, genomes,
recurrent memories, the immigrant genome template, mutation parameters, population
target, and ID counters.
Animal IDs serialize as integers; loading validates each complete animal and
the remaining experiment state. Native CBOR arrays hold weights;
no safetensors payload is used.

Locks, channels, pacing clocks, and viewer/device resources live outside Universe.
Reconstructible caches may be omitted from serialization; recurrent memory and RNG
positions are required state. Core simulation and genetics contracts have no
persistence bounds.

The current format is checkpoint version 5, with `foraging-sense-first-v1` and
`animal-rnn-3-4-3-v1` model identifiers. Only this format is accepted;
compatibility and migrations are intentionally unsupported. Rust module/type names
are not part of the format.

Only the simulation owner saves, between complete ticks. Snapshot copying holds
the Universe's read lock; serialization and file I/O happen after releasing
it. A file is written to a temporary sibling, flushed and synced, then renamed
into place. Older checkpoints are removed only after the new file is published.
An OS lock permits one writer per directory and releases automatically when the
process exits. Temporary files from an incomplete write are ignored on resume.

The newest checkpoint is selected by tick. Schema/model versions and restored
state invariants are checked before running or replacing configuration. Invalid or
truncated checkpoints produce an error instead of silently restarting. To recover
an older retained checkpoint, move newer checkpoint files out of `checkpoints/`
while the run is stopped. The format has no checksum.
Hard kills or power loss can lose progress since the last completed save; an
exit checkpoint requires orderly shutdown.

Exact replay targets the same build/platform. The [Universe checks](../src/universe/tests.rs)
and [CLI checks](../tests/persistence_cli.rs) cover complete-state continuation
and resume across separate processes.
