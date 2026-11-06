# Run persistence

Pass a directory to create or resume a run; omit it to create a timestamped folder
under `runs/`. `--latest` selects the most recently saved run there. Selection
errors never silently start a fresh run or fall back to another directory.
Keep screenshots and analysis alongside checkpoints, outside Git.

Resume loads the saved resolved configuration, recovering it from the checkpoint
if `config.toml` is missing. Explicit [overrides](configuration.md) may change
runtime settings, but must preserve seed and model parameters. A saved tick budget
applies again on every invocation; remove `ticks` from the resolved configuration
for unlimited continuation.

Checkpoints are saved periodically and on orderly shutdown, including Ctrl-C.
A hard kill can lose progress since the last completed save. Only one process may
write a run directory. Saves publish complete ticks atomically; retention removes
old checkpoints only after publishing the new one. Interrupted temporary writes
are ignored. Without any completed checkpoint, initialization uses the saved seed.

The [persistence module](../src/app/persistence.rs) defines supported format/model
identifiers and checkpoint layout. Unsupported state formats are rejected; model
changes must not silently reinterpret an old run. Obsolete diagnostic and smoke
settings are discarded only when loading saved configurations. New overlays
reject obsolete keys.

Invalid checkpoints fail before replacing saved files. To recover an older retained
checkpoint, stop the run and move newer files out of `checkpoints/`. Metrics history
is independent and remains unchanged, including overlapping ticks after recovery.
The format has no checksum or recovery past internal corruption.

[Simulation contracts](simulation.md) define continuation guarantees.
[Metrics](metrics.md) explains invocation files and fresh windows after resume.
