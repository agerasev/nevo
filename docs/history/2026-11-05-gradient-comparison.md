# Evolved RNNs versus a gradient follower

**November 5, 2026.** Test whether the final controllers from the
[million-tick run](2026-11-05-long-run.md) outperform a controller that follows
the sensed food gradient at maximum speed. Zero gradient means no movement.

## Protocol

Use the original `b86d5bd` ecosystem, with 32 paired seeds (1–32) and 20,000
ticks each. Each initial world has 500 mature plants and 26 animals at energy
100. The RNN population contains every saved genome once, shuffled across
positions independently for each seed, with recurrent memory reset. Both arms
start from identical serialized physical state and RNG streams. Subsequent
worlds diverge through different feeding and birth events.

Immigration and mutation are disabled. Reproduction remains enabled, with
unchanged inherited controllers; genotype frequencies can change. This evaluates
the saved mixed population, not each genotype in isolation. Original movement
has no additional energy cost. No current simulation rules were modified.

## Results

Both controllers survived all 32 trials without immigration. Across ticks
10,001–20,000, means over matched seeds were:

| Measurement | Evolved RNNs | Gradient follower |
| --- | ---: | ---: |
| Mean population | 24.990 | 24.976 |
| Food energy per tick | 49.959 | 49.909 |
| Births per 1,000 ticks | 31.191 | 35.488 |
| Deaths per 1,000 ticks | 31.063 | 35.447 |
| Movement distance per active animal-tick | 9.681 | 9.988 |

Late food collection favored RNNs by 0.050 energy/tick, approximately 0.10%
(paired bootstrap 95% interval 0.027–0.073). The whole-trial difference was
0.009 (interval −0.002–0.020). Both approach the 50-energy/tick production limit;
initial ripe food explains the early transient above it.

RNNs had about 12% lower birth/death turnover and traveled about 3% less. Late
deaths were lower in 31/32 pairs; the paired difference was −4.384 per 1,000
ticks (interval −5.166 to −3.594). These are exploratory marginal intervals
from 20,000 bootstrap resamples of paired world seeds.

The simple controller nearly matches the evolved population's food collection
and supported population. This environment is largely solved by direct gradient
following; the RNNs show lower turnover, without a substantial throughput
advantage. The resource ceiling limits this comparison's ability to expose
controller differences. Results do not establish competition outcomes, lifetime
differences, robustness beyond 20,000 ticks, or behavior with movement costs.

## Provenance

Artifacts and archived harness copies are in
`runs/2026-11-05-controller-comparison/`. The three scripts are preserved in Git
at `0972d51:scripts/`; they target simulation revision `b86d5bd`, not the current
model. Artifacts include the protocol, source hashes, states, logs, summaries,
and `comparison.png`.

A held-out seed-0 pair reproduced identical states and logs over 1,000 ticks.
All 64 trajectories passed population accounting and continuity checks; RNN
genomes remained copies of founders. No fresh-random-genome comparison was run.
