# Simulation contracts

A [Universe](../src/universe/mod.rs) owns complete reproducible state: environment,
actor, execution context, intervention policy, identity allocator, and step counter.
It serializes directly to CBOR. Recurrent memory and RNG positions are state;
future reconstructible caches may use serde's skip/default mechanism.

## Components

| Contract | Responsibility |
| --- | --- |
| [Actor](../src/actor/mod.rs) | Accept typed observations, then produce an action. Observation handling can update state without scheduling an action. |
| [Population](../src/actor/population/mod.rs) | An Actor containing other actors. Route inputs and lifecycle observations; create controllers through a replaceable Factory. |
| [Environment](../src/environment/mod.rs) | Own physical state; produce observations and apply actions, physical lifecycle rules, and requested external interventions. |
| [Policy](../src/universe/mod.rs) | Decide experiment interventions from environment requests. The Universe owns and invokes it. |
| [Mind](../src/mind/mod.rs) | Process internal signals without knowing bodies, food, or movement. |
| [Genome / Mutate](../src/genome/mod.rs) | Clone inherited representations and mutate them through an execution context. |
| [Context](../src/context/mod.rs) | Supply RNG and execution resources; the CPU context owns the genetic RNG. |

`Actor` replaces the draft's `Agent` name and separates observation handling from
acting. `Genome`, `Mutate`, `Mind`, and `Context` retain their contracts.
`Herbivore<M>` maps food observations to a compatible mind's three inputs and maps
three outputs to movement. `Rnn` owns a 47-parameter genome and four recurrent units;
all units read the previous hidden state. Processing consumes no random draws.

## Identity and lifecycle

The ecosystem owns `Plant`s and `Animal`s; the population owns controllers in
separate storage. Opaque `Handle`s serialize as integers. A Universe-owned monotonic
allocator never reuses them. Ascending handle order retains feeding priority.
There are no serialized pointers or pointer reconstruction on load.

The concrete population protocol carries inputs or ordered spawn/despawn changes,
plus a complete membership roster. The population checks that roster against its
controllers. Loading validates components and their observation protocol at the
Universe boundary. Handles prevent arbitrary construction through the regular API;
validated associations, rather than shared ownership, keep the two stores aligned.

Physical births and deaths come from the environment as lifecycle observations.
The factory interprets `Origin::Child { parent }` by copying and mutating the parent
genome; the new mind starts with zero memory. Default weight mutation adds independent
noise in [-0.02, 0.02). Invalid bounds fail; zero preserves the RNG stream.

Initialization is immigration into an empty environment. Both startup and later
arrivals use `Ecosystem::admit` with placement and energy parameters, and
`Origin::Immigrant` creates their controllers. An optional immigrant genome template
applies to every such arrival; otherwise each receives random weights. The
[foraging setup and policy](../src/universe/foraging.rs) choose the initial layout
and minimum population. The environment only performs physical admission.

## A complete step

The [runner](../src/run/mod.rs) holds the Universe read lock while preparing an
independent next state. It routes observations to the actor, obtains actions,
advances the environment with the policy's intervention responses, and delivers
lifecycle feedback to the actor. It then releases the read lock and publishes the
complete next Universe under a write lock. The Universe does not interpret births,
deaths, or genomes.

Viewers can read the previous complete state during preparation. A failure discards
the prepared state, including controller memory, RNG draws, policy changes, and
allocated handles. The runner is the sole writer; its `UniverseView` exposes only
read access. Cloning a Run creates an independent simulation; cloning its view
attaches a reader. The first implementation clones complete state each step; this
cost is accepted for a clear atomic boundary. Replacement components must clone
independently mutable state, not share mutable simulation state through pointers.

The ecosystem senses bodies with nonnegative energy in stable handle order, then
moves and clamps them. Feeding and culling follow. Eaten plants are replaced; the
policy sees the surviving population count and requests immigrants. Reproduction
then splits eligible animals, followed by upkeep, plant growth, and final clamping.
Lifecycle feedback initializes controllers in the same step, but newborns and
immigrants first act on the next step. Decisions record sensing-time energy and
movement endpoints before birth offsets and final clamping.

The default policy maintains 20 animals; the debug policy injects no replacements.
The debug layout admits one animal at energy 390 beside a ripe plant. At seed 1 it
senses before eating, then splits; both animals finish step one at energy 213.
The baseline admits 20 animals among 500 ripe plants. Both use clamped boundaries
and food-field strength plus normalized world-coordinate gradient as perception.

## Execution and viewing

One `Run::execute` loop accepts a per-invocation tick limit, pacing interval,
command receiver, and after-step stop predicate. A trial is initialization plus a
limit; an unlimited run omits it. Pacing defaults to full speed. SplitMix64 expands
the master seed into separate spatial and genetic Xoshiro256++ streams. Rendering
and wall time never affect draws or physics; replay targets one build/platform.

Commands apply between complete steps. Single-step pauses and advances once;
its acknowledgement follows publication. Pause/resume reset pacing. Limits,
predicates, stop commands, or dropped senders finish the loop. Errors return the
last published state and mark the loop failed. A fresh channel can resume a Run.

The viewer independently clones the complete Universe under its read lock and
renders its environment. Toolbar and arena show the same step. Closing the window
stops and joins the worker. `--smoke` compares twelve displayed Universe snapshots,
including minds and RNGs, with a reference run and checks pause/step commands.
[Viewer](../src/app/viewer/mod.rs) · [Locking and failure checks](../src/run/tests.rs) ·
[Continuation checks](../tests/simulation_runs.rs) · [Persistence](persistence.md)
