# Foraging interfaces

Use one world tick for short herbivore trials and continuous ecology. The July 26
experiment is the agreed Rust baseline. The interfaces and presets below are
proposals. [Historical rules and recordings](../experiments/2016-07-26-herbivore-foraging/README.md)

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AgentId(u64);

trait Controller {
    type Observation;
    type Action;
    fn act(&mut self, obs: &Self::Observation) -> Self::Action;
}

trait Population<Observation, Action, Origin> {
    fn spawn(&mut self, id: AgentId, origin: Origin);
    fn act(&mut self, id: AgentId, obs: &Observation) -> Action;
    fn despawn(&mut self, id: AgentId);
}

trait Environment<P> {
    type Report;
    fn tick(&mut self, population: &mut P) -> Self::Report;
}
```

`ForagingWorld` implements `Environment<P>` for any population that accepts its
observation, action, and origin types. It owns the plants, bodies, energy ledger,
stable IDs, and spatial rules. The population maps each living ID to an inherited
genome and a controller. A founder can use a supplied debug genome, an immigrant
gets a random genome, and an offspring copies and mutates its named parent's
genome. Each starts with fresh controller memory. A scripted controller and a
recurrent brain can consume the same observations. A different body or sensor can
supply its own observation encoder and action decoder.

**Reference preset.** `FoodObservation` carries field strength and the two
components of its normalized gradient in world coordinates. `MoveAction` carries
a direction and speed fraction. The original network had four recurrent units
and three outputs for these actions. Initially only its 47
weights and biases vary. Use clamped edges for the historical comparison. Record
wrapping and energy-as-input as named variations. Energy is available in the debug
trace for every preset. With wrapping, wrap positions after movement and use the
shortest wrapped displacement for both sensing and contact.

**One tick.** Resolve feeding at current positions in stable ID order and remove
dead bodies and controllers. Replace eaten plants, admit immigrants, and split
eligible parents. Register new controllers immediately. Apply upkeep and plant
growth, then ask every living controller for an action against the same world
state. Apply movement after gathering all actions. The newborn acts on its birth
tick. Report meals, births with parent ID, immigration, and deaths. A focal trace
can also record observation, action, position, and energy. Keep world and genetic
random draws in separately seeded streams so controller experiments do not shift
food draws just by consuming more randomness.

**Two runners.** A trial constructs fresh world and population state from a layout,
genomes, and seeds. It can stop on a focal animal's death or after a tick budget
and record which occurred. A continuous run constructs that pair once and keeps
it alive across reporting intervals. Both call `world.tick(&mut population)`.
To continue the exact trial trajectory, keep the same pair after the trial budget.
Immigration, reproduction, and any external selection are explicit experimental
settings. The viewer consumes state and events at its own frame rate.

**First debug case.** Put a herbivore with 390 energy in contact with one ripe
plant and disable immigration in this fixture. Its meal adds 40, the split leaves
215 in each body, and upkeep leaves 213 in each. The trace should show two decisions
in that tick. With identical initial state, policies, and seeds, an N-tick trial
should match the first N ticks of a continuous run, including events and recurrent
state. This checks both the phase order and the boundary between the runners.
