# NEvo

An artificial-life and neuroevolution research project in Rust. Animals with
recurrent brains forage among replenishing plants. The aim is measurable
adaptation across increasingly demanding environments; survival alone is not
evidence of adaptation.

Animals steer by forward acceleration and turning, with persistent heading and
velocity under viscous drag. Movement consumes energy through mechanical work
and motor losses. Animals and plants share collision physics with fixed obstacles.

Initialize the submodules with `git submodule update --init`, then:

```sh
# Start a seeded, bounded experiment.
cargo run --locked -- runs/baseline --config configs/world/small-uniform.toml --seed 1 --ticks 256
# Continue the saved experiment with a viewer.
cargo run --locked -- runs/baseline --config configs/viewer/window.toml
# Inspect a small deterministic scene.
cargo run --locked -- --config configs/world/debug.toml --config configs/diagnostics/trace.toml --viewer --paused
# Explore resource patches, physical contacts, and fixed obstacles.
cargo run --release --locked -- \
  --config configs/world/large-nonuniform.toml \
  --config configs/obstacles/scattered.toml \
  --config configs/brain/small.toml \
  --config configs/mutation/adaptive.toml \
  --config configs/topology/evolving.toml \
  --config configs/brain-upkeep/weight-count.toml \
  --config configs/sensing/plants-and-animals.toml \
  --config configs/locomotion/damped.toml \
  --config configs/movement/work-and-loss.toml --viewer --paused
```

Run directories contain resolved configuration, checkpoints, and metrics.
`--ticks` is an additional budget for each invocation; omit it on a new run for
unlimited execution. See `--help` for CLI shortcuts and [persistence](docs/persistence.md)
for resuming. Build with `--no-default-features` when graphics are unnecessary.

The design keeps minds and environments replaceable through small contracts.
Experiment-specific sensing, actions, bodies, and genomes stay outside the generic
runner. Simulation time is independent of viewing and wall-clock pacing; wgame
provides the optional drawing backend. The [simulation contracts](docs/simulation.md)
explain the boundaries.

- [Configuration](docs/configuration.md): composable presets and energy units.
- [Metrics](docs/metrics.md): measurements and interpretation.
- [Viewer](docs/viewer.md): controls and drawing contract.
- [Research roadmap](docs/roadmap.md): planned milestones and unresolved choices.
- [Selected history](docs/history/README.md): measured results and their limits.

Keep reusable presets in `configs/` and generated artifacts in ignored `runs/`.
[Working instructions](AGENTS.md) cover code, tests, and documentation policy.
