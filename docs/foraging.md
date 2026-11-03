# Foraging interfaces

The headless simulation uses one world tick for short herbivore trials and
continuous ecology. The three [Rust traits](../src/lib.rs) separate a controller's
decision, a population's lifecycle, and an environment's rules.
[Historical rules and recordings](../experiments/2016-07-26-herbivore-foraging/README.md)

`ForagingWorld` implements `Environment<P>` for any population that accepts its
observation, action, and origin types. It owns the plants, bodies, energy ledger,
stable IDs, and spatial rules. The population maps each living ID to an inherited
genome and a controller. A founder can use a supplied debug genome, an immigrant
gets a random genome, and an offspring copies and mutates its named parent's
genome. Each starts with fresh controller memory. A scripted controller and a
recurrent brain can consume the same observations. To run a known founder, pass
`Herbivores::with_founder_genome` a genetic seed and genome when building the pair.
[World](../src/foraging.rs) · [Brains and inheritance](../src/herbivore.rs)

**Reference controller.** `FoodObservation` carries field strength and the two
components of its normalized gradient in world coordinates. `MoveAction` carries
a direction and speed fraction. The four recurrent units and three outputs use
47 inherited `f32` weights and biases. A child receives independent uniform noise
of up to 0.02 per parameter. All recurrent units read the previous hidden state
in Rust. The C++ matrix multiply overwrote parts of that state during the update.
Both Rust scenarios use clamped edges and food-only inputs. The debug trace also
reports each body's energy and positions before and after movement.

**One tick.** Resolve feeding at current positions in stable ID order and remove
dead bodies and controllers. Replace eaten plants, admit immigrants, and split
eligible parents. Register new controllers immediately. Apply upkeep and plant
growth, then ask every living controller for an action against the same world
state. Apply movement after gathering all actions. The newborn acts on its birth
tick. Report meals, births with parent ID, immigration, and deaths. A focal trace
records observation, action, positions, and energy. A master seed initializes
SplitMix64, whose first two outputs seed separate Xoshiro256++ streams for
spatial draws and genetics. Replay comparisons apply to the same build/platform.

**Two runners.** A trial constructs fresh world and population state from a layout,
genomes, and seeds. It can stop on a focal animal's death or after a tick budget
and record which occurred. A continuous run constructs that pair once and keeps
it alive across reporting intervals. Both call `world.tick(&mut population)`.
To continue the exact trial trajectory, call `tick` again on the same pair.
`check` initializes independent runs, compares reports and complete state after
every trial tick, then resumes the trial and compares one additional tick.
[Runners and comparison](../src/run.rs)

**Debug fixture.** A herbivore with 390 energy starts in contact with one ripe
plant. Immigration is disabled in this fixture. Its meal adds 40, the split
leaves 215 in each body, and upkeep leaves 213 in each. Parent and child both
act that tick. The full baseline replenishes the population with random immigrants
whenever it falls below 20. [Integration checks](../tests/foraging_runs.rs)
