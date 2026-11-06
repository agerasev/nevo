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

Bodies retain heading, linear velocity, and angular velocity. The current mind
observes the food field and its own motion in body coordinates, then requests
bounded forward and angular acceleration. Drag damps both velocities. This keeps
the controller's observations and actions independent of the world's compass
axes. Fixed mass and moment of inertia are separate from energy reserves.
At a split, both resulting bodies retain the parent's heading and linear and
angular velocities, without an extra birth motion charge. They separate in
opposite directions along a uniformly sampled angle around the former position.
Each mind chooses its own commands on the next tick.

The [body integrator](../src/ecosystem/body/mod.rs) advances physics in `f64`
substeps, with one mind update and one set of ecological events per tick. Arena
walls clamp position and remove outward normal velocity. Animal collisions and
obstacle contacts belong to the next milestone. `phy` was evaluated but its
current `f32` state would require a precision adapter; the current integrator
remains local to the ecosystem. Movement budgets and time units are described in
[configuration](configuration.md).

The [runner](../src/run/mod.rs) is the sole state writer. Readers see complete
published ticks; a failed or discarded computation must leave that state intact.
Rendering, metrics, and wall-clock pacing must not affect simulation trajectories
or random streams. Reproducibility targets the same build and platform.

Checkpoint state must include everything that affects continuation, including
body heading and velocities, recurrent memory, and RNG positions. Derived caches
may be rebuilt, but rebuilding must preserve the trajectory. See
[persistence](persistence.md), the focused
[continuation invariant](../src/universe/tests.rs), and
[publication invariants](../src/run/tests.rs).
