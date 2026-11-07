# Research roadmap

Completed milestones are marked below; the remaining sequence and mechanisms are
recommendations. **Open** choices require agreement before implementation.
The [controller comparison](history/2026-11-05-gradient-comparison.md) reached the
food-production ceiling, while [costly movement](history/2026-11-05-costly-movement-models.md)
slowed reproductive turnover. Prioritize environments where navigation and
behavioral tradeoffs can pay off before adding evolutionary complexity.

Each milestone should yield a runnable preset and relevant invariants, validated
with an explicit tick budget.
Steps 1–4 establish navigation; 5–6 extend ecology. Mutation mechanisms in 7–9
are implemented; their evaluation needs an established environment.

1. **Completed: larger worlds and camera navigation.** [Camera controls](viewer.md)
   and fixed [sensing scales](configuration.md) support a
   [large uniform world](../configs/world/large-uniform.toml).
2. **Completed: persistent resource patches.** Fixed Gaussian landscapes set local
   capacity; smooth crowding limits seedling recruitment with bounded sampling and
   configurable recovery. [Composable presets](configuration.md) separate
   replenishment from individual plant growth; [metrics](metrics.md) distinguish
   plant production from food intake.
3. **Completed: body heading and damped locomotion.** Bodies
   retain heading and velocities, with forward and angular acceleration controlled
   in body coordinates. [Locomotion and movement presets](configuration.md)
   separate physical dynamics from positive mechanical work and motor losses.
   [Physics substeps](simulation.md) preserve one mind update and one set of
   ecological events per tick. Heading is visible in the viewer.
4. **Local perception and obstacles.** Contact physics completed and validated:
   animals and plants share disk dynamics with fixed obstacles and geometry-aware
   admission. **Next:** finite-range body-relative rays reporting nearest surfaces
   and an explicit miss, with obstacle occlusion and optional food-gradient smell.
   **Open:** field of view, ray count, categories, smell rules, and wrapping-boundary
   rollout. Compare modalities on held-out maps; masking an input and evolving
   without it answer different questions.
5. **Nonlinear plant growth.** Compare fast recovery with slow seedling / fast
   intermediate / slow mature growth. Restraint in harvesting is a hypothesis.
   **Open:** curve, timescale, and regeneration. Logistic growth cannot restart
   zero-energy replacements; account for seed reserves or other introduced energy.
6. **Predator–prey ecology.** Begin with fixed herbivore/carnivore diets and evolving
   controllers, reusing bodies and perception. **Open:** attacks, escapes, costs,
   energy transfer, and kin/cannibalism rules. Establish simple-controller behavior
   before interpreting coevolution; distinguish coexistence from immigration support.
7. **Completed: Gaussian mutations.** Independent normal noise applies to weights
   and biases. The initial scale matches the previous uniform noise variance.
   **Evaluation remains open:** compare fixed-scale Gaussian mutation against the
   earlier uniform implementation; a different distribution need not improve adaptation.
8. **Completed: inherited mutation strength.** One positive scale per animal's
   genome changes in log space before weight mutation, with a configurable fixed
   log-space variation. [Mutation presets](configuration.md) support adaptive,
   fixed-scale, and disabled mutation. **Evaluation remains open:** compare fixed
   and inherited scales. Defer per-matrix/per-weight scales and evolution of the
   adaptation rate. Small scales may be selected in stable tasks; self-adaptation
   does not guarantee continued exploration.
9. **Completed: evolving hidden width with brain upkeep.** Bounded neuron
   additions/removals preserve surviving connections. Upkeep scales with stored
   weight and bias count; [presets](configuration.md) separate initial width,
   topology mutation, and upkeep. [Metrics](metrics.md) report hidden widths and
   brain energy expenditure. **Evaluation remains open:** compare fixed and
   evolving widths across upkeep costs. New capacity costs energy before it becomes
   useful; larger brains are not a success criterion. Compare genetic coordinates
   only across compatible shapes.

Preserve the [component contracts](simulation.md) and [persistence policy](persistence.md).
Keep a runnable reference and use replicated evolutionary seeds with held-out
matched worlds. Control starting energy, reset recurrent memory, disable mutation
and immigration during controller evaluation, and state whether reproduction remains
active. Specify whether evaluation samples individuals, lineages, or populations.

Separate resource supply from controller performance. Measure task-relevant energy,
reproduction, survival, and behavior before drawing evolutionary conclusions.
Keep artifacts in `runs/` and concise measured results in [history](history/README.md).
