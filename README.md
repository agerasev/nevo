# NEvo

An ongoing artificial-life and neuroevolution experiment in Rust: evolving agents
acting in simulated environments. Development proceeds through runnable milestones
and recorded experiments, with no final endpoint for the research program.

## Current implementation

The ecosystem has replenishing plants and animals with 3–4–3 recurrent
brains. One configurable loop runs every experiment; an optional desktop
viewer reads the shared Universe. Sensing precedes movement, feeding, and births.
A fresh checkout needs `git submodule update --init` for the Cargo path dependency.

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/viewer.toml
cargo run --locked -- --config configs/debug.toml --config configs/viewer.toml --paused
cargo run --locked -- --config configs/baseline.toml --seed 1 --ticks 256
cargo run --locked --bin nevo -- runs/baseline --seed 1 --ticks 256
cargo run --locked --bin nevo -- runs/baseline
```

Use **Pause / Resume** to control playback and **Single step** to advance one tick
and pause. With the arena focused, Space toggles playback and Right Arrow steps.
The toolbar shows the seed, tick, and population counts for the displayed
world. Green circles are plants and yellow circles are animals.

Runs default to full speed (`--hz 0`) and a seed drawn from system entropy.
The resolved seed is printed before the run starts; supply `--seed N` to replay it.
Use `--hz 25` for paced viewing and `--ticks N` for a reproducible budget. Omit
the budget to run until interrupted. A trial simply supplies initialization and
a tick limit. The `debug.toml` preset exercises a meal and a birth on tick one. The
`baseline.toml` preset starts 20 animals among 500 mature plants.
See the [tick and runner contracts](docs/simulation.md).

CLI and TOML configuration share one [conf](https://docs.rs/conf/0.4.5/conf/)
schema. Repeat `--config PATH` to layer presets: later files override earlier ones,
and CLI values override all files. Tables merge by field; arrays replace as a whole.
[Example configuration](configs/debug.toml):

```toml
seed = 1
trace_ticks = 3

[ecosystem]
initial_animals = 1
plants = 1
minimum_population = 0
initial_animal_energy = 390.0
initial_animal_position = [0.0, 0.0]
initial_plant_position = [0.0, 0.0]
```

Run options use snake_case in TOML and kebab-case on the CLI. Ecosystem options
use `[ecosystem]` in TOML and `--ecosystem-…` on the CLI. Booleans accept bare flags
or explicit values such as `--viewer=false`. Unknown keys are rejected in every
file. See [configuration and precedence](docs/configuration.md).

Every run stores its resolved configuration and resumable CBOR checkpoints in
the supplied directory, or a new directory under `runs/`. Pass that directory
alone to resume with its last configuration. Checkpoints are saved periodically
and on orderly shutdown, including Ctrl-C; their interval and retention count
are configurable. `--ticks N` always means N additional ticks for that invocation.
See [persistence options, format, and recovery](docs/persistence.md).

Build with `--no-default-features` when graphics are unnecessary; the executable
and runner stay the same. Add `--smoke` to a viewer command to verify twelve
displayed snapshots and pause/step controls against a seeded reference run.

## Project structure

Component contracts live at their module roots; implementations live beneath them.
Multi-file modules use `mod.rs` inside their directory.

| Module | Responsibility |
| --- | --- |
| [mind](src/mind/mod.rs) | Mind contract, food-sensing adapter, and scalar RNN with memory and genome. |
| [genome](src/genome/mod.rs) | Genome, Mutate, and independent RandomInit contracts. |
| [context](src/context/mod.rs) | Execution context contract and CPU implementation. |
| [ecosystem](src/ecosystem/mod.rs) | Plants and complete animals with body and mind; foraging, reproduction, and immigration rules. |
| [universe](src/universe/mod.rs) | Complete serializable state and the small Simulation stepping contract. |
| [run](src/run/mod.rs) | Shared read/prepare/write loop and controls. |
| [viewer](src/viewer/mod.rs) | Optional Draw contract, wgame scenes, and generic window controls. |
| [app](src/app/mod.rs) | CLI configuration, run storage, worker and viewer startup. |

`main.rs` only enters the application; `lib.rs` exposes the library modules.
The concrete experiment is `ecosystem::Ecosystem`. Each animal owns its `body` and
`mind` together. A `Universe` owns the complete experiment and step counter;
both bounded and unlimited runs use `run::Run`. Initialization and later immigration
share one admission path. Minds receive typed observations and return actions;
there is no separate environment–actor storage or lifecycle synchronization.
Each tick prepares component updates under a read lock and applies them under a
write lock, without cloning the Universe or existing minds.
See [simulation contracts](docs/simulation.md) for the implemented boundaries.

Objects compose their default drawing through `Draw`; the viewer collects a wgame
scene under a read lock and renders after releasing it. Viewing requires no model
or mind cloning. See [drawing and viewer contracts](docs/viewer.md).

Keep current documentation in [docs/](docs/README.md), with selected milestone
conclusions in [history](docs/history/README.md). Run configurations live in
`configs/`; checkpoints, screenshots, and other run outputs stay in ignored `runs/`.

## Agreed direction

- Use one configurable loop for resettable trials and unlimited experiments;
  the optional viewer reads snapshots independently of simulation stepping.
- Make agents (genomes, brains, control mappings) and environments (worlds, bodies,
  physical rules) generic and replaceable through compatible contracts/adapters.
- Use wgame as the drawing backend, with one default presentation per object.
- Use the [July 26 foraging experiment](docs/history/2016-07-26-ecosystem.md)
  as the Rust baseline. Start with continuous 2D space, replenishing food, and one
  animal body type. Wrapping boundaries remain part of the planned Rust world.
- Include internal-state perception. World perception remains an open design:
  investigate a 1D angular retina with color and distance, and possibly gradient
  sensing as smell, with modality ablations.
- Begin with the minimum evolutionary degrees of freedom needed to demonstrate
  adaptation. The first baseline evolves weights and biases with fixed topology.
- Record concise conclusions from major research milestones in [history](docs/history/README.md).

## Proposed design

These are working proposals, not implemented interfaces or settled parameters.

- A simple retina can use fixed body-relative rays, reporting the nearest surface's
  color and normalized distance with an explicit miss value. Specify field of
  view, range, resolution, and occlusion before implementing it.
- Compare vision-only, smell-only, and combined perception using matched trial
  seeds and budgets. A first ablation can mask input channels while retaining the
  same network architecture; training without a modality and disabling it only
  during evaluation answer different questions.
- Record code/configuration/seeds with results. Target repeatability within the
  same build/platform first; a seeded RNG alone is not a portability promise.

Open choices include retina and smell definitions, which internal signals to
expose, and how to select genomes across trials.
