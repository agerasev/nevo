# NEvo

An ongoing artificial-life and neuroevolution experiment in Rust: evolving agents
acting in simulated environments. Development proceeds through runnable milestones
and recorded experiments, with no final endpoint for the research program.

## Current implementation

The simulation code on `novel` as of `88cf113` draws a seeded 100 x 100 world with 1,000 plants
and 100 animals. [src/main.rs](src/main.rs) initializes and redraws the scene;
movement, perception, reproduction, and evolution are not implemented here yet.

The stack is Rust 2024, `glam`, `rand`, and the [wgame](wgame/README.md) graphics
submodule. The existing `--smoke` option exits after 12 rendered frames.

## Agreed direction

Decisions from 2026-11-03:

- Support controlled, resettable trials for debugging and evaluation first, then
  long-running continuous ecology, sharing the simulation machinery.
- Make agents (genomes, brains, control mappings) and environments (worlds, bodies,
  physical rules) generic and replaceable through compatible contracts/adapters.
- Start with continuous 2D space, wrapping boundaries, replenishing food, and one
  herbivore body type. Expand the ecosystem through later experiments.
- Include internal-state perception. World perception remains an open design:
  investigate a 1D angular retina with color and distance, and possibly gradient
  sensing as smell, with modality ablations.
- Begin with the minimum evolutionary degrees of freedom needed to demonstrate
  adaptation. Exact genome parameters and controller architecture remain open.
- Keep a concise history of experiments in [experiments/](experiments/README.md).

## Proposed design

These are working proposals, not implemented interfaces or settled parameters.

- Retain the current graphics stack; use `wgame-egui` for controls. Keep the
  simulation CPU-based, fixed-step, and runnable without a window.
- Separate environment/body state, observation encoding, controller/brain memory,
  action decoding, inherited genome, and experiment orchestration. A world owns
  its physical semantics; a controller need not know its concrete world or body.
  Small typed contracts should enable replacement without making every field
  generic or requiring all environments to expose energy or a fitness score.
- Separate reset policy from selection policy: episodic trials may test unchanged
  agents or evaluate genomes for generational selection; continuous ecology may
  select through births and deaths. An episode is not necessarily a generation.
- Initially evolve only weights of a small fixed-topology network. Keep body,
  sensor geometry, and action meanings fixed within each experiment. Inheritance
  should distinguish genome data from an individual's transient neural memory.
- A simple retina can use fixed body-relative rays, reporting the nearest surface's
  color and normalized distance with an explicit miss value. Specify field of
  view, range, resolution, and occlusion before implementing it.
- Compare vision-only, smell-only, and combined perception using matched trial
  seeds and budgets. A first ablation can mask input channels while retaining the
  same network architecture; training without a modality and disabling it only
  during evaluation answer different questions.
- Record code/configuration/seeds with results. Target repeatability within the
  same build/platform first; a seeded `SmallRng` alone is not a portability promise.

Next discussion: observation/action contracts, retina and smell definitions,
smallest useful brain/genome, and trial selection and ecological energy rules.

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
