# Food seeking with recurrent motor oscillations

**November 6 run; analyzed November 7, 2026.** Run
`runs/2026-11-06_16-13-48` ended at tick 2,428,910 with 80 animals. These
6–16–2 brains control thrust and angular acceleration through persistent body
motion. This experiment predates adaptive mutation and evolving brain width.

## A successful family inside an immigration-supported population

The final population contains one close genetic family of 47 animals, one older
unrelated animal, and 32 animals belonging to recent immigrant lineages
(31 immigrants and one offspring). Pooling all 80 as evolved controllers would
obscure the result.

In the final 18,910 ticks, the dominant family accounted for 55.1% of animal-time,
86.6% of food intake, and 92 of 93 births. Its food intake per animal-tick was
5.27 times that of the remaining population; it also had 91 deaths. These are
observations within one shared ecosystem, not a controlled estimate of genetic
advantage.

The full run recorded 10,806 births and 134,078 immigrants. Average food income
was 0.0361 energy per tick, below approximately 0.0400 upkeep plus 0.00990 motor
cost. Immigration supplied 0.0138 energy per tick. The floor of 80 therefore
concealed a substantial energy deficit; this does not imply that the dominant
family could not sustain a smaller population.

## Interpretable controller rules

- **Steer toward food and thrust mainly when facing it.** The food gradient was
  ahead of the dominant family on 88.9% of actual animal-ticks. Sampled mean
  thrust was 42.3% within 45° of ahead and 3.4% within 45° of directly behind.
- **Use moderate, pulsed effort.** Mean thrust was 35.7%; it was completely off
  on 28.5% of ticks. All 47 brains oscillated under a constant front-food probe;
  42 settled into a two-tick motor cycle.
- **Steering is asymmetric.** Fixed-input probes produced a left-turn bias with
  food straight ahead. Actual median food bearing was about 13° right of the
  heading. Stronger food signals increased thrust in the tested front-food
  probes. Motion feedback modulated commands, but angular-velocity feedback
  was not consistently damping.

These rules come from probing the whole network, including its recurrent state,
rather than assigning meanings to individual weights. Oscillation establishes
temporal dynamics, not useful long-term memory or planning.

## Recurrence ablation without immigration

Continue the final checkpoint for 20,000 ticks with immigration and mutation
disabled. Compare intact brains with their recurrent contribution set to zero
every tick. In the main comparison, retain only the 47 related animals, with
identical bodies, resources, obstacles, and initial random state in both worlds.

| Dominant family only | Intact recurrence | Recurrence disabled |
| --- | ---: | ---: |
| Mean active population | 50.24 | 68.44 |
| Final population | 50 | 70 |
| Food collected | 711.54 | 835.46 |
| Motor energy spent | 208.62 | 140.89 |
| Births | 88 | 200 |

Removing recurrence collected 17.4% more food and spent 32.5% less motor energy,
despite supporting 36.2% more animal-time. The initial comparison retaining all
80 animals agreed in direction: 16.0% more food and 34.8% less motor energy.

The brains implement useful food seeking, but recurrence reduced performance
in these continuations. This is one starting world, not independent replication;
it does not establish that recurrence is generally harmful or isolate motor
oscillation from other changes in the controller. The
[earlier gradient-follower comparison](2026-11-05-gradient-comparison.md) used
different sensors and movement rules and does not rank these brains.

## Evidence and reproducibility

The run metadata records `e82d885-dirty`. Source archived from `6840261`, with
matching submodule revisions, reproduced the complete persisted final state
from tick 2,410,000 exactly. NumPy one-step probes agreed with native actions
within 3.87e-7. Metrics passed window-continuity and population-accounting checks.
Command averages use every replayed tick; detailed input probes use every 19th
tick to avoid sampling only one phase of a two-tick cycle.

The run's ignored `analysis/` directory holds the archived source, replay and
ablation executables, `analyze.py`, `plot.py`, numerical results, observations,
and figures. Production simulation code was not changed for this review.
