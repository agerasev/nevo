# Simulation contracts

One [runner](../src/run/mod.rs) advances every experiment. A viewer is an optional
reader of that run. A trial is an initial configuration plus a stopping condition;
it can use the same world size and rules as an unlimited experiment.

**Agent and genome contracts.** `agent::Agent<C>`, `genome::Genome<C>`,
`genome::Mutate<C, P>`, and `context::Context` separate processing, inheritance,
mutation, and execution resources.
`Agent::process` takes a context and an owned input and returns a fallible output.
`Genome` requires cloning; `Mutate::mutate` changes a genome using parameters and
the context's RNG. The [CPU context](../src/context/cpu.rs) wraps the genetic RNG;
device and dtype metadata are unit values for native Rust scalar execution.
The scalar RNN consumes no random draws during processing.

**Mind contract.** `mind::Mind<C>` processes its own associated input/output types
without knowing about bodies, food, or movement. `mind::rnn::Rnn` implements it
for three scalar inputs and outputs, owning `RnnGenome` and recurrent memory.
`Herbivore<M>` maps the ecosystem's observation/action types to these signals and
can wrap any compatible mind through `Herbivore::from_mind`. It propagates mind
errors to its caller. [Mind](../src/mind/mod.rs) · [RNN](../src/mind/rnn/mod.rs)

`Population` owns agent lifetimes and decisions. Its concrete `Herbivores` adapter
unwraps processing and mutation results: this agent always succeeds and the fixed
mutation parameters are valid. A future backend that can fail needs an explicit
population error policy. The population still chooses a concrete herbivore type;
the contracts do not provide a generic agent factory or sexual reproduction.
[Agent contracts](../src/agent/mod.rs) · [Population and inheritance](../src/agent/herbivore/population.rs)

**World contract and locking.** `Environment<P>::observe` returns owned, typed
observations with stable agent IDs. `Run::tick` collects these under a read lock,
releases it, then asks the population for actions. `Environment<P>::update` applies
the responses and lifecycle changes under a write lock. `Run` is the sole writer;
its cloneable `WorldView` only exposes read access. Agents execute without a world
lock, and readers never see a partially updated tick. Cloning a complete `Run`
creates an independent simulation; cloning its view attaches another reader.

**Ecosystem tick.** Observe all bodies with nonnegative energy in stable ID order.
Compute their actions, then move and clamp those bodies. Resolve feeding at the
new positions, remove dead bodies and agents, replace eaten plants, admit
immigrants, and split eligible parents. Apply upkeep and plant growth, then clamp
bodies and plants again to account for growth and birth offsets. Newborns and
immigrants first act on the following tick. Decisions record sensing-time energy
and movement endpoints before birth offsets or the final clamp.

[World rules](../src/environment/ecosystem/mod.rs) · [Order and locking checks](../tests/run_loop.rs)

**Reference agent.** Food field strength and its normalized gradient in world
coordinates feed a fixed 3–4–3 recurrent network with 47 inherited `f32` parameters.
Outputs specify direction and speed fraction. All recurrent units read the
previous hidden state. Both scenarios use clamped edges and food-only inputs.
Children clone the parent genome and call `Mutate` with `WeightMutation::default()`,
adding independent noise in [-0.02, 0.02) per parameter. Custom bounds must be
nonnegative with a finite sampling range; zero is a no-op. New agents start with
fresh memory. `Herbivores::with_founder_genome` supplies a known founder genome;
immigrants still receive random genomes.

**One loop.** `Run::execute` accepts an optional tick limit, a pacing interval,
a command receiver, and an after-tick stop predicate. The budget applies to that
invocation and leaves the live run available for continuation. Pacing defaults
to zero (full speed). Per-tick rules and RNG draws never depend on
rendering or elapsed wall time. SplitMix64 expands the master seed into separate
Xoshiro256++ streams for spatial draws and genetics. Replay targets one build and
platform. [Continuation checks](../tests/simulation_runs.rs)

Commands take effect between ticks. Pause/resume reset the pacing delay, and
single-step pauses and advances exactly once. Blocking command acknowledgements
arrive after application; a step acknowledgement follows the completed update.
Stop, the tick limit, and a true predicate finish the loop. Dropping all command senders
also stops it. A fresh channel can resume the same `Run` in another invocation.

**Viewer.** A separate window loop copies a snapshot under the read lock, releases
it, and renders the snapshot. The toolbar and arena display the same tick. UI
controls send commands to the simulation worker; drawing never advances it.
Completion leaves the final state visible. The application stops and joins its
worker when the window closes. `--smoke` checks twelve displayed snapshots and
control acknowledgements, starting from either a fresh or restored run.
Circle strokes are constructed before scale and
translation to preserve their centers. [Viewer](../src/app/viewer/mod.rs)

**Persistence.** The worker saves complete state between ticks and reconstructs
a restored run with `Run::from_parts`. This does not introduce another stepping
loop. [Storage policy and snapshot contents](persistence.md)

**Debug fixture.** One herbivore starts with 390 energy in contact with a ripe
plant. It senses that plant before its first action. In seed 1 it then eats for
40 energy and splits; both bodies finish at 213 after upkeep. Only the parent
acts on tick one. Immigration is disabled. The baseline starts 20 founders and
500 ripe plants, replenishing random immigrants whenever population falls below
20. [Behavioral checks](../tests/simulation_runs.rs)
