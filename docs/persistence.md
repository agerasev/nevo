# Run persistence

Each invocation uses a run directory. Supply its path as the positional argument;
omit it without `--latest` to create `runs/YYYY-MM-DD_HH-MM-SS/` using local time.
If the name is taken, append `-2`, `-3`, etc. to create a fresh directory.
The chosen path is printed to stderr. Keep screenshots and other run outputs
alongside checkpoints in that directory, for example under `screenshots/`.
All generated run data stays out of Git.

```sh
# Create a run with the default checkpoint policy.
cargo run --locked -- runs/baseline --seed 1 --ticks 256

# Resume from its newest checkpoint, loading config.toml automatically.
cargo run --locked -- runs/baseline

# Resume the most recently saved run under runs/.
cargo run --locked -- --latest

# Continue for 64 more ticks.
cargo run --locked -- runs/baseline --ticks 64
```

`--latest` checks immediate subdirectories of `runs/`, including named runs, and
selects the run with the most recently modified checkpoint file. Directory names
break equal-time ties. Artifact folders and runs without checkpoints are ignored.
The selected run uses the usual saved configuration and explicit overrides.
No saved runs, a locked directory, or an invalid selected checkpoint produces an
error; it never silently starts fresh or falls back to another run.
`--latest` and an explicit directory are mutually exclusive.

`config.toml` records the last effective configuration, including the resolved
entropy seed. Ordered `--config PATH` files override individual settings, then CLI
values override the files. See [configuration precedence](configuration.md).
If `config.toml` is missing, the checkpoint's embedded configuration is recovered.
Resume rejects changed seeds or ecosystem model parameters. Runtime settings
such as pacing, viewer attachment, tracing, save policy, and metrics can change.
A tick budget always applies to the current invocation: a saved budget of 256 means another 256 ticks on bare
resume. Remove `ticks` from `config.toml` for an unlimited continuation.

Checkpoint policy belongs in config files. For example, load this with `--config PATH`
to save only on exit and retain one checkpoint:

```toml
checkpoint_every = 0
keep_checkpoints = 1
```

| TOML | Default | Meaning |
| --- | --- | --- |
| `checkpoint_every` | `10000` | Save at absolute world ticks divisible by N; zero disables periodic saves. |
| `keep_checkpoints` | `3` | Retain the newest N checkpoints; must be positive. |

The worker also saves when its budget ends, the viewer closes, or a stop signal
arrives (Ctrl-C; also SIGTERM/SIGHUP on Unix). Saving the same tick replaces that
checkpoint. A zero-tick invocation still saves. There is no automatic initial
checkpoint; if interrupted before its first save, a directory with only a config
and an optional metrics log reinitializes from that saved seed. Recorded history remains.

## Layout and consistency

```text
runs/baseline/
  config.toml
  .lock
  metrics.cborseq          # enabled by default; appends across invocations
  checkpoints/
    00000000000000000100.cbor
    00000000000000000200.cbor
    00000000000000000256.cbor
```

Each checkpoint is one CBOR file containing the effective configuration,
model identifiers, and the directly serialized
[Universe](../src/universe/mod.rs): step and complete experiment model. The ecosystem
owns ordered plants and animals (each with body and mind), both RNG states, genomes,
recurrent memories, the immigrant genome template, physical and genetic parameters,
population target, and ID counters. Runtime settings cannot replace these canonical
rules when resuming.
Animal IDs serialize as integers; loading validates each complete animal and
the remaining experiment state. Native CBOR arrays hold weights;
no safetensors payload is used.

Locks, channels, pacing clocks, and viewer/device resources live outside Universe.
Metric caches are omitted from serialization and start fresh after loading;
recurrent memory and RNG positions are required state. Finalized statistics remain
in the single [CBOR metrics log](metrics.md), which is never replayed into checkpoints.
Core simulation and genetics contracts have no persistence bounds.

The current format is checkpoint version 9, with `foraging-sense-first-v3` and
`animal-rnn-3-n-3-v2` model identifiers. Only this format is accepted;
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
