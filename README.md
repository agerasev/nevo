# NEvo

An ongoing artificial-life and neuroevolution experiment in Rust: evolving agents
acting in simulated environments. Development proceeds through runnable milestones
and recorded experiments, with no final endpoint for the research program.

## Current implementation

The Rust baseline uses the July 26 foraging experiment with a 3–4–3 recurrent
herbivore agent. One configurable loop runs every experiment; an optional desktop
viewer reads the shared world. Sensing now precedes movement, feeding, and births.
A fresh checkout needs `git submodule update --init` for the Cargo path dependency.

```sh
cargo run --locked --bin nevo -- --viewer --scenario baseline --seed 1
cargo run --locked --bin nevo -- --viewer --scenario debug --seed 1 --paused
cargo run --locked --bin nevo -- --scenario baseline --seed 1 --ticks 256 --hz 0
```

Use **Pause / Resume** to control playback and **Single step** to advance one tick
and pause. With the arena focused, Space toggles playback and Right Arrow steps.
The toolbar shows the seed, scenario, tick, and population counts for the displayed
world. Green circles are plants and yellow circles are herbivores.

All commands accept `--ticks`, `--seconds`, and `--hz` (default 25; zero removes
pacing). Omit limits to run until interrupted. A trial simply uses an initial
configuration and a limit. Use `--focal 1` to trace a founder and stop on its death.
`--seconds` counts wall time, including pauses; tick limits give reproducible
budgets. The `debug` layout exercises a meal and a birth on tick one. The
`baseline` starts 20 herbivores among 500 mature plants.
See the [tick and runner contracts](docs/foraging.md).

Build with `--no-default-features` when graphics are unnecessary; the executable
and runner stay the same. Add `--smoke` to a viewer command to verify twelve
displayed snapshots and pause/step controls against a seeded reference run.

## Agreed direction

Decisions through 2026-11-04:

- Use one configurable loop for resettable trials and unlimited experiments;
  the optional viewer reads snapshots independently of simulation stepping.
- Make agents (genomes, brains, control mappings) and environments (worlds, bodies,
  physical rules) generic and replaceable through compatible contracts/adapters.
- Use the [July 26 foraging experiment](experiments/2016-07-26-herbivore-foraging/README.md)
  as the Rust baseline. Start with continuous 2D space, replenishing food, and one
  herbivore body type. Wrapping boundaries remain part of the planned Rust world.
- Include internal-state perception. World perception remains an open design:
  investigate a 1D angular retina with color and distance, and possibly gradient
  sensing as smell, with modality ablations.
- Begin with the minimum evolutionary degrees of freedom needed to demonstrate
  adaptation. The first baseline evolves weights and biases with fixed topology.
- Keep a concise history of experiments in [experiments/](experiments/README.md).

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
  same build/platform first; a seeded `SmallRng` alone is not a portability promise.

Open choices include retina and smell definitions, which internal signals to
expose, and how to select genomes across trials.

## Historical context

Git contains three independent roots; the newer versions are fresh starts.

| Lineage | Useful reference |
| --- | --- |
| 2016 C++/Qt | `origin/master` at `b96e415`: plants, herbivores, carnivores, recurrent brains, energy, reproduction, weight mutation, and champion selection. The fullest ecological implementation in source. |
| 2016 forks | `origin/cpp` has an earlier herbivore simulation; `origin/win32` preserves a simpler Qt4 prototype; `origin/python` is an unfinished PyQt4/NumPy port. |
| 2022 Rust | Root `27ee89a`; `4b957d4` has custom recurrent networks, weight/memory-size mutations, and an egui animal viewer. World stepping is empty. |
| 2024-2025 Rust | `origin/develop` at `b5af8cf`: Candle convolutional vision and LSTM brains, deterministic initialization, and a grid viewer. World stepping and animal control remain unimplemented. |
| 2025-2026 Rust | Root `a5f1138` starts `novel` with `wgame`; `88cf113` updates dependencies and rendering. |

The recurring implementation lesson is to connect perception, action, resources,
and inheritance early while keeping the boundaries replaceable.
