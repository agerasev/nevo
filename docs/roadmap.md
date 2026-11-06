# Research roadmap

Completed milestones are marked below; the remaining sequence and mechanisms are
recommendations. **Open** choices require agreement before implementation.
The [controller comparison](history/2026-11-05-gradient-comparison.md) reached the
food-production ceiling, while [costly movement](history/2026-11-05-costly-movement-models.md)
slowed reproductive turnover. Prioritize environments where navigation and
behavioral tradeoffs can pay off before adding evolutionary complexity.

Each milestone should yield a bounded runnable preset and relevant invariants.
Steps 1–4 establish navigation; 5–6 extend ecology. Step 7 can happen earlier;
8–9 need an established evaluation environment.

1. **Completed: larger worlds and camera navigation.** [Camera controls](viewer.md)
   and fixed [sensing scales](configuration.md) support a bounded
   [larger-world preset](../configs/large-world.toml) with unchanged initial densities.
2. **Completed: persistent resource patches.** Fixed Gaussian landscapes set local
   capacity; smooth crowding limits seedling recruitment with bounded sampling and
   configurable recovery. [Composable presets](configuration.md) separate
   replenishment from individual plant growth; [metrics](metrics.md) distinguish
   plant production from food intake.
3. **Body heading and damped locomotion.** Give bodies persistent heading, linear
   velocity, and angular velocity. Use body-relative sensing/control, initially
   bounded forward thrust and turning torque with drag. **Open:** sideways/reverse
   thrust, force versus acceleration, mass/inertia, drag, effort costs, and birth
   momentum. Effort should cost energy even without displacement. Evaluate `phy`
   as an integrator; precision and checkpoint integration need resolution first.
   Physics substeps must not multiply ecological events/rates or rerun brains.
   Rotating a scene should preserve body-frame inputs and rotate world-frame actions.
4. **Local perception and obstacles.** Start with finite-range body-relative rays
   reporting nearest surfaces and an explicit miss, plus optional food-gradient
   smell. Static obstacles occlude vision; contacts and spawning respect geometry.
   **Open:** field of view, ray count, categories, smell rules, contact method,
   animal collisions, and wrapping-boundary rollout. Integration alone does not
   prevent tunneling. Compare modalities on held-out maps; masking an input and
   evolving without it answer different questions.
5. **Nonlinear plant growth.** Compare fast recovery with slow seedling / fast
   intermediate / slow mature growth. Restraint in harvesting is a hypothesis.
   **Open:** curve, timescale, and regeneration. Logistic growth cannot restart
   zero-energy replacements; account for seed reserves or other introduced energy.
6. **Predator–prey ecology.** Begin with fixed herbivore/carnivore diets and evolving
   controllers, reusing bodies and perception. **Open:** attacks, escapes, costs,
   energy transfer, and kin/cannibalism rules. Establish simple-controller behavior
   before interpreting coevolution; distinguish coexistence from immigration support.
7. **Gaussian mutations.** Compare Gaussian and uniform noise at matched variance,
   including biases: uniform `[-a, a)` corresponds to `sigma = a / sqrt(3)`.
   Keep topology and scale fixed initially; a different distribution need not improve
   adaptation.
8. **Inherited mutation strength.** Start with one positive scale per genome,
   mutated multiplicatively by log-normal noise before applying weight mutations.
   **Open:** initial scale, fixed log-space variation, and numerical bounds.
   Compare fixed and inherited scales; defer per-matrix/per-weight scales and
   evolution of the adaptation rate. Small scales may be selected in stable tasks;
   self-adaptation does not guarantee continued exploration.
9. **Evolving hidden width with brain upkeep.** First compare fixed widths, then
   infrequent neuron additions/removals preserving existing connections where
   possible. **Open:** size limits, mutation rates, initialization, and neuron versus
   connection costs. Dense recurrence grows quadratically; new capacity costs energy
   before it becomes useful. Keep input/output semantics fixed and compare genetic
   coordinates only across compatible shapes. Larger brains are not a success criterion.

Preserve the [component contracts](simulation.md) and [persistence policy](persistence.md).
Keep a runnable reference and use replicated evolutionary seeds with held-out
matched worlds. Control starting energy, reset recurrent memory, disable mutation
and immigration during controller evaluation, and state whether reproduction remains
active. Specify whether evaluation samples individuals, lineages, or populations.

Separate resource supply from controller performance. Measure task-relevant energy,
reproduction, survival, and behavior before drawing evolutionary conclusions.
Keep artifacts in `runs/` and concise measured results in [history](history/README.md).
