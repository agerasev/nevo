# Research and implementation roadmap

## Status and motivation

This is the working plan from the November 6, 2026 discussion, not a description
of implemented behavior. The requested directions are richer spatial ecology,
an animal's own heading, more sensing, predation, and more adaptable evolution.
The ordering and concrete mechanisms below are recommendations; choices marked
**Open** need agreement before their implementation. Current behavior remains in
[simulation](simulation.md), [configuration](configuration.md), and [viewer](viewer.md).

The [gradient-follower comparison](history/2026-11-05-gradient-comparison.md)
found little food-throughput advantage for evolved brains near the resource
ceiling. The [costly-movement run](history/2026-11-05-costly-movement-models.md)
also shows why many ticks need not mean many reproductive opportunities.
Prioritize environments where navigation and behavioral tradeoffs can pay off;
better mutation or larger brains alone do not establish richer adaptation.

## Implementation sequence

Each step should end with a bounded runnable preset, relevant checks, updated
current documentation, and a commit. Steps 1–4 build a spatial-navigation
baseline. Steps 5–6 extend its ecology. Step 7 is independent and can happen
earlier; steps 8–9 should use an established evaluation environment.

### 1. Larger worlds and camera navigation

- Use existing configurable world dimensions; add drag-to-pan, cursor-centered
  zoom, and fit-world controls. Keep camera state in the generic viewer and use
  the existing wgame drawing path.
- Separate physical sensing distance from arena dimensions and signal normalization
  from total plant count. Preserve animal size, movement units, and local sensor
  scale when enlarging the world. Initially scale plant count with area to separate
  world size from scarcity; distant plants must not dilute local signal strength.
- **Check:** pan/zoom and resizing preserve the intended world point; fit restores
  full bounds; viewing never changes simulation state. Compare equivalent local
  scenes in different arena sizes to check sensing scale.

### 2. Persistent resource patches

- Add a seeded fertility field with productive patches and poorer gaps. Configure
  plant placement density and local growth multipliers separately. Apply spatial
  placement rules to replacements as well as initial plants.
- Start with simple growth. Record regional food production, depletion, discovery,
  and travel costs; compare layouts with comparable total food supply.
- **Open:** patch representation and scale; weighted respawning versus persistent
  plant sites. Stable patches are recommended, but individual plants need not
  occupy fixed sites.
- **Check:** reproducible layouts and continuation; replacements preserve the
  configured distribution; growth and harvested energy accounting agree.

### 3. Body heading and damped locomotion

- Give `Animal.body` a persistent heading, linear velocity, and angular velocity.
  Heading remains defined at rest and independent of sliding velocity. Express
  observations and actions in body coordinates; expose own energy, forward/sideways
  velocity, and angular velocity to `AnimalMind`.
- A proposed minimal model is `dx/dt = v`,
  `dv/dt = a_control - v / tau + a_contact`, with analogous angular drag.
  Bounded forward thrust and turning torque are the recommended initial controls.
  Drag supplies a finite response time and terminal speed.
- Extend the scalar brain's input/output shape and validation for the new adapter;
  keep a fixed hidden width initially. Put experiment-specific channel meanings in
  `AnimalMind`, not the generic `Mind` or `Simulation` contracts.
- **Open:** forward-only propulsion versus sideways/reverse thrust; acceleration
  versus force commands; body mass/inertia, drag, propulsion energy costs, and
  heading/momentum at birth and immigration. Effort should cost energy even when
  an animal pushes without moving.
- Evaluate [phy](https://github.com/agerasev/phy) as an integrator. It supplies
  Euler/RK4, not collision detection or contact resolution. Resolve precision and
  checkpoint-state integration before adopting it; its local built-in numerical
  types currently differ from NEvo's `f64` physical state.
- Hold body-frame commands fixed during physics substeps, rotating thrust into
  world coordinates using the current intermediate heading. Do not run brains,
  feeding, births, or random draws during derivative evaluations, or multiply
  ecological rates by substep count. Keep physical time independent of viewer pacing.
- **Check:** drag decay, terminal motion, braking/turning, stationary heading,
  energy expenditure, and checkpoint continuation. Rotating positions, headings,
  velocities, and surroundings together must preserve body-frame inputs and
  rotate world-frame actions. This is rotational equivariance; a particular map
  can still have directional structure.

### 4. Local perception and obstacles

- Add finite-range angular sensing for plants, other animals, and obstacle
  geometry. Recommended starting point: body-relative rays reporting the nearest
  surface's distance and color/category, plus an explicit miss value. Obstacles
  occlude vision. Keep optional local food-gradient sensing as a separate smell
  modality, with its own range and obstruction rules.
- Add simple static obstacles and circular moving bodies, with low-bounce
  contacts. Use sensing and contacts together so navigation is possible. Keep
  births, immigration, and plant placement out of solid obstacles.
- **Open:** field of view, ray count, normalization, visible categories, smell
  rules, contact method, and animal–animal collisions. Soft spring/damper contacts
  permit overlap and require suitable substeps; harder contacts need explicit
  collision handling. RK4 alone does not prevent tunneling through obstacles.
- Wrapping boundaries remain planned. Decide their rollout consistently across
  movement, contacts, sensing, and drawing; current clamping is not wrapping.
- **Check:** range/occlusion and rotated scenes; wall sliding, corners, maximum
  speed versus thin obstacles, and valid spawn locations. Benchmark larger
  populations and add spatial indexing when needed, checking queries against a
  simple reference implementation.
- Run a navigation comparison across held-out maps: patch discovery, detours,
  trapping, energy efficiency, reproduction, and survival. Compare vision-only,
  smell-only, and combined inputs. Masking inputs at evaluation and evolving
  without a modality answer different questions; hold architecture fixed when
  isolating sensing effects.

### 5. Nonlinear plant growth

- Add an explicit growth curve composable with the fertility field. Compare fast
  recovery from depletion with slow seedling / fast intermediate / slow mature
  growth. The latter could make early harvesting reduce future productivity;
  evolved restraint is a hypothesis, not an expected result.
- **Open:** curve, maturation timescale, and regeneration mechanism. Logistic
  `r * E * (1 - E/K)` cannot grow current zero-energy replacements. Options include
  seed reserves, unconsumed roots, or age-based maturation; account for any energy
  introduced at replacement.
- **Check:** zero/mature states, energy bounds, supply accounting, and matched
  growth-budget comparisons. Measure harvest maturity and regional productivity.

### 6. Carnivores and predator–prey ecology

- Begin with fixed herbivore/carnivore diets and evolving controllers. Reuse body,
  movement, and sensing mechanisms; make prey and predators perceptually
  distinguishable. Use terrain to provide pursuit routes and possible refuges.
- **Open:** attack action/range, escape mechanics, costs, prey-to-predator energy
  transfer, and kin/cannibalism rules. Evolvable diet and combat traits are later
  possibilities, not prerequisites for this step.
- **Check:** contact/attack ordering and energy accounting; evaluate simple
  controllers before interpreting coevolution. Measure both populations,
  encounters, kills, escapes, extinction, and any immigration separately.
  Coexistence and richer behavior must be measured across maps and seeds.

### 7. Gaussian weight mutations

- Add normally distributed noise as an explicit alternative to uniform mutation,
  retaining fixed topology and mutation scale for the first comparison.
- Match variance: uniform `[-a, a)` corresponds to Gaussian `sigma = a / sqrt(3)`;
  current `a = 0.02` gives approximately `0.01155`. A Gaussian sigma of `0.02`
  would triple the mutation variance. Include biases consistently.
- **Check:** seeded reproducibility, distribution scale, finite values, and
  zero-mutation behavior. Compare distribution shapes at matched variance over
  several evolutionary seeds; do not presume Gaussian noise is better.

### 8. Inherited mutation strength

- Start with one positive scale per genome, used at reproduction:
  `sigma_child = sigma_parent * exp(tau * z)`, then
  `w_child[i] = w_parent[i] + sigma_child * z_i`, with independent standard-normal
  draws. Keep `tau` fixed initially. This mutates scale proportionally to itself;
  it changes mutation magnitude, not the probability of selecting a weight.
- **Open:** initial scale, fixed log-space variation, and numerical bounds. Compare
  fixed versus inherited scales. Per-matrix and then per-weight scales are later
  alternatives; they add evolutionary degrees of freedom and require separate
  evidence. Do not recursively evolve the scale's adaptation rate initially.
- **Check:** offspring inheritance without changing the parent, shared initialization
  for initial/later immigrants, positivity, bounds, and continuation. Record scale
  distributions and descendant success; very small scales may be selected in a
  stable environment. Self-adaptation does not guarantee continued exploration.
- Background: [Beyer and Schwefel, evolution strategies](https://gwern.net/doc/reinforcement-learning/model-free/2002-beyer.pdf).

### 9. Evolving hidden width with brain upkeep

- First compare fixed hidden widths on the established tasks. Then introduce
  infrequent neuron additions/removals, preserving existing connections where
  possible. Width is already inherited; structural mutation and upkeep are new.
- **Open:** size limits, structural mutation rates, new-neuron initialization, and
  neuron/connection energy costs. Dense recurrence grows quadratically, so decide
  whether upkeep prices neurons, connections, or both. New capacity pays a cost
  before becoming useful; weak initial influence limits disruption but does not
  remove that tradeoff.
- Input/output semantics remain fixed for each experiment. Remap matrix blocks
  explicitly on width changes, including any per-weight mutation scales. Report
  width distributions and compare coordinate-based diversity only within
  compatible shapes.
- **Check:** shape validity, inheritance, structural changes, brain costs, and
  continuation. Compare against fixed-width controls; larger brains are not a
  success criterion, and shrinking can be appropriate for a simple task.

## Shared implementation and experiment rules

- Preserve the existing [component contracts](simulation.md) and independent
  stepping/rendering. Use explicit composable settings and presets under
  `configs/`; new nested settings do not imply new CLI flags.
- Before changing state or neural shapes, update model/checkpoint identifiers and
  follow the [persistence policy](persistence.md). Do not silently reinterpret old
  runs or promise migration. Save heading, velocities, genomes, recurrent state,
  and all other state required for exact continuation.
- Keep a runnable reference for comparisons. Validate same-build replay,
  checkpoint continuation, behavioral invariants, and headless execution as
  affected by each change. Add metrics needed for that step before long runs.
- Record revision, resolved configuration, seeds, tick budgets, births, and
  lineage depth when available. Use replicated evolutionary runs and held-out
  matched worlds; control initial energy and reset recurrent memory when comparing
  controllers. Evaluation should disable mutation and immigration and explicitly
  state whether reproduction remains enabled. Specify how individual genomes,
  representative lineages, or whole populations are selected for evaluation;
  these answer different questions.
- Separate changes in resource supply from controller performance. Report energy,
  reproduction/turnover, survival, and task-specific behavior alongside population.
  Keep artifacts in `runs/`; place only concise measured conclusions in
  [history](history/README.md). No step implies a successful evolutionary outcome.
