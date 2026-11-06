# Simulation contracts

The runner must support experiments with different entities, sensing, actions,
and inherited representations. Keep those semantics in the experiment and its
adapters, rather than extending universal interfaces for each new model.

[Simulation and Universe](../src/universe/mod.rs) define the state and update
contract; [Mind](../src/mind/mod.rs) defines inference; [genome contracts](../src/genome/mod.rs)
separate inheritance, mutation, and initialization. Their Rust documentation is
the authoritative home for computation, application, and configuration guarantees.
A mind need not have a genome, and a simulation need not support metrics or drawing.

The current [ecosystem](../src/ecosystem/mod.rs) owns complete animals, each with a
body and mind. Thinking must leave the inherited genome unchanged; recurrent or
learned state belongs to the mind. [AnimalMind](../src/mind/animal/mod.rs) owns the
mapping between biological observations/actions and neural signals. Initial and
later immigration follow the same admission policy.

The [runner](../src/run/mod.rs) is the sole state writer. Readers see complete
published ticks; a failed or discarded computation must leave that state intact.
Rendering, metrics, and wall-clock pacing must not affect simulation trajectories
or random streams. Reproducibility targets the same build and platform.

Checkpoint state must include everything that affects continuation, including
recurrent memory and RNG positions. Derived caches may be rebuilt, but rebuilding
must preserve the trajectory. See [persistence](persistence.md), the focused
[continuation invariant](../src/universe/tests.rs), and
[publication invariants](../src/run/tests.rs).
