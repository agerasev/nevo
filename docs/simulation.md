# Simulation contracts

A [Universe](../src/universe/mod.rs) owns the step counter and one complete experiment
model. The model owns its physical state, minds, RNGs, and experiment rules. Universe
serializes directly to CBOR; locks and the viewer live outside it.

## Components and ownership

| Contract or type | Responsibility |
| --- | --- |
| [Simulation](../src/universe/mod.rs) | Compute an owned update, apply it, and validate an experiment, without prescribing its entities or sensing. |
| [Ecosystem](../src/ecosystem/mod.rs) | The foraging model: plants, complete animals, perception, physics, population support, and random streams. |
| `Animal<B, M>` | Own a body and its mind together: `body: B`, `mind: M`. |
| [Mind](../src/mind/mod.rs) | Compute typed output and a state update from input; apply the update and validate mind state. |
| [Genome / Mutate](../src/genome/mod.rs) | Clone inherited representations and mutate them using an execution context. |
| [RandomInit](../src/genome/mod.rs) | Initialize a value from the context RNG, independently of Genome. |
| [Context](../src/context/mod.rs) | Supply RNG and execution resources; the CPU context owns the genetic RNG. |
| [Metrics / Aggregate](metrics.md) | Collect typed component summaries and reduce scalar observations. |

`Ecosystem<M, G, P>` can use another compatible mind, genome, and mutation parameter
type. Genetic minds expose their genome through `AsRef<G>` and construct fresh minds
through `TryFrom<G>`; the baseline provides infallible `From<RnnGenome>` conversions.
In this ecosystem, thinking leaves the inherited genome unchanged; learned state
belongs to the mind's update. The Mind contract itself does not require a genome.
A different experiment implements `Simulation`; the runner imposes no animal,
population, observation, or action types. `Animal` is generic over its body and mind;
the foraging model uses its own physical `Body` type.

The ecosystem stores animals in one ordered map. A birth constructs both body and
mind; death removes the whole animal. Opaque `AnimalId`s serialize as integers and
are never reused. They identify events, parents, and animals, without joining
separate body and mind stores. Population is this collection, with no separate
Actor, Environment, lifecycle-message protocol, or universal intervention callback.

Explicit mind inputs still define what an animal perceives. `AnimalMind<M>` maps
food observations to three neural signals and three brain outputs to movement;
its `brain` can be any compatible Mind. The scalar RNN owns 47 inherited parameters
and four recurrent units, all reading the previous hidden state. Minds receive
observations, not unrestricted access to the world. The RNN consumes no random draws
when processing.

Immigration clones the optional genome template or calls `RandomInit::random`, then
constructs a fresh mind. Reproduction clones the parent's genome, calls `Mutate` with
the experiment's stored parameters, and constructs the offspring's mind. There is no
separate inheritance-policy object. Both paths propagate initialization, mutation,
and conversion errors through Universe computation.

For the baseline, random genomes draw 47 weights in [-0.5, 0.5). Default mutation
adds independent noise in [-0.02, 0.02), and new brains start with zero recurrent
memory. Invalid mutation bounds fail when used; zero preserves the RNG stream.
Initial and later arrivals both use `Ecosystem::admit` with physical admission
parameters. A supplied genome template applies to all immigrants without consuming
genetic random draws.

## A complete step

The [runner](../src/run/mod.rs) holds a Universe read lock while `compute_update`
computes an owned update. It releases that lock, then installs the update and step
counter under a write lock. Other readers can inspect the previous complete state
during computation. The runner is the sole writer; `UniverseView` exposes only reads.

`Simulation` and `Mind` remain separate contracts with consistent method names.
`compute_update(input, context)` returns `(output, update)`; Simulation
uses the tick as input and owns its execution context internally. `apply_update`
consumes the update. `Simulation::Report` and `Mind::Output` are observations of the
computation; their `Update` types contain changes to commit.

Each contract has an associated `Config` and a `configure` setup hook. Creation
and `Universe::restore(config)` configure components before running; step methods
use stored settings. Mind setup receives its own config and the parent's metrics
enable flag, which gates diagnostics without changing behavior. New animals use
the ecosystem's stored mind settings. Configuration follows component ownership
without a per-entity map. Direct `Universe::advance()` needs no configuration.
`Run::from_universe` wraps an already configured Universe; cloning a Run preserves
its current runtime caches rather than treating the clone as a resume.

Computation performs all fallible work, validates the proposed state, and uses
private RNG copies. Failure or a discarded update leaves published state unchanged.
Application consumes a successful update on its unchanged source state; it cannot
fail, draw randomness, or run further model computation. Computation must not change
simulation state through interior mutability. Stepping requires no `Clone` bound on
the model or mind.

The ecosystem works on body and plant copies to preserve sequential feeding and
birth rules. Its update carries final physical state, mind updates, new animals,
deaths, RNG positions, and ID counters. Existing minds and genomes stay in place:
the RNN computes recurrent memory and optional diagnostics, and reproduction copies
a genome only for the child. Immutable experiment parameters are borrowed during computation.
Updates are transient; metric caches are omitted from the serialized Universe.

Cloning a Run still creates an independent simulation; cloning its view attaches a
reader. Persistence copies complete state; viewing collects drawing instances.
Reconstructible caches may be omitted from serialization; recurrent memory and RNG
positions are required state.

The ecosystem senses animals with nonnegative energy in stable ID order and obtains
all actions before moving any bodies. Movement and clamping precede feeding and
culling. Eaten plants are replaced, immigrants restore the configured minimum
population, and eligible animals reproduce. Upkeep, plant growth, and final clamping
complete the tick. Newborns and immigrants first act on the following tick. Decisions
record sensing-time energy and movement endpoints before birth offsets/final clamping.

The baseline preset admits 20 animals among 500 ripe plants and maintains a minimum
of 20. The debug preset admits one animal at energy 390 beside a ripe plant, with no later
immigration. At seed 1 it senses before eating and then splits; both animals finish
step one at energy 213. These are [configurations](configuration.md) of the same model,
with independent counts, starting energy, and placements. Both use clamped boundaries
and food-field strength plus its normalized world-coordinate gradient as perception.

## Execution and viewing

One `Run::execute` loop accepts a per-invocation tick limit, pacing interval,
command receiver, and after-step stop predicate. A trial is initialization plus a
limit; an unlimited run omits it. Pacing defaults to full speed. SplitMix64 expands
the master seed into separate spatial and genetic Xoshiro256++ streams. Rendering
and wall time never affect draws or physics; replay targets one build/platform.

Commands apply between complete steps. Single-step pauses and advances once;
its acknowledgement follows publication. Pause/resume reset pacing. `SetInterval`
changes the wall-clock interval and wakes a pacing wait without changing pause
state; `LoopStatus` reports the applied interval. Limits,
predicates, stop commands, or dropped senders finish the loop. Errors return the
last published state and mark the loop failed. A fresh channel can resume a Run.

The viewer collects geometry and labels through `Draw` under a read lock, then
releases it before UI layout and rendering. Toolbar labels and arena describe the
same state; normal viewing never clones the Universe or its minds. Closing the
application window stops and joins the worker. The `smoke` config setting compares the complete
state used for twelve displayed scenes, including minds and RNGs, with a reference
run and checks pause/step/pacing commands.
[Viewer contract](viewer.md) · [Locking and failure checks](../src/run/tests.rs) ·
[Continuation checks](../tests/simulation_runs.rs) · [Persistence](persistence.md)

## Statistics

Independent [Metrics and Aggregate contracts](metrics.md) compose typed summaries.
Components own optional caches, computed with their updates and omitted from CBOR.
Windows restart after resume; metrics never change simulation rules or random draws.
