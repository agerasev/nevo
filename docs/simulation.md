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
axes. Animals and plants are physical disks: positive reserve energy sets mass
through the common `mass_per_energy` scale, and inertia is `0.5 * mass * radius²`.
Plants have passive linear and angular drag. Zero-energy plants remain physically
inactive until they grow; animals with nonpositive reserves are culled next tick.

The [shared dynamics](../src/ecosystem/physics/mod.rs) use `phy`'s `f64` RK4
integrator and `geom2`'s disk contact geometry. Contacts between moving disks,
arena walls, and fixed infinite-mass obstacles use non-attractive normal spring
and damping forces. Contacts are compliant and allow temporary overlap. Reduced
mass scales disk-pair response. Fixed RK4
substeps resolve motion and contacts, while minds and ecological events run once
per tick. Spatial bins accelerate pair lookup without discretizing positions or
contact geometry. Movement budgets and timestep settings are described in
[configuration](configuration.md).

[Assimilation](../src/ecosystem/feeding.rs) joins the edible fraction of a plant
to the animal's center of mass, linear momentum, and angular momentum, including
orbital momentum. The discarded fraction leaves with its own momentum; this
transfer adds no metabolic fee. Plant growth adds biomass with zero momentum,
diluting velocity and spin. Upkeep and motor reserve expenditure preserve the
body's velocities as its mass changes.

At a split, both resulting bodies retain the parent's heading and linear and
angular velocities, without an extra birth motion charge. They separate in
opposite directions along a uniformly sampled angle around the former position.
This preserves linear momentum, while the smaller disk inertias reduce total
spin angular momentum. Each mind chooses its own commands on the next tick.

The [runner](../src/run/mod.rs) is the sole state writer. Readers see complete
published ticks; a failed or discarded computation must leave that state intact.
Rendering, metrics, and wall-clock pacing must not affect simulation trajectories
or random streams. Reproducibility targets the same build and platform.

Checkpoint state must include everything that affects continuation, including
animal and plant motion, recurrent memory, and RNG positions. Derived caches
may be rebuilt, but rebuilding must preserve the trajectory. See
[persistence](persistence.md), the focused
[continuation invariant](../src/universe/tests.rs), and
[publication invariants](../src/run/tests.rs).
