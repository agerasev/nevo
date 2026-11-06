# Flat versus energy-proportional movement costs

**November 5, 2026.** Test whether charging more for heavier animals' movement
reduces jitter. Revision `ef6bb71`, seed `12981673030976906931`, two fresh runs of
300,000 ticks. Initial animals, plants, positions, genomes, memory, and RNG states
matched exactly; only the movement coefficient and energy power differed.
Evolution and immigration remained enabled, with 500 plants and population floor 20.

Energy uses the default reproduction threshold of 1. Both conditions pay upkeep
0.0005 per tick. Flat movement adds
`0.00045 × distance`; energy-proportional movement
adds `0.0009 × pre-movement energy × distance`. At energy 0.5 their costs match.

## Measured results

The comparison interval was fixed in advance: ticks (200,000, 300,000].
Population and energy means include all ticks and animals. Heading changes compare
consecutive controlled displacements of the same animal, each longer than 1e-9;
stops break the sequence and birth offsets are excluded. Full replays matched both
saved final states exactly; event totals also matched the recorded metrics.

| Measurement, final 100,000 ticks | Flat cost | Energy-proportional cost |
| --- | ---: | ---: |
| Mean population | 34.87 | 64.66 |
| Mean animal energy | 0.4852 | 0.3317 |
| Food energy per tick | 0.124978 | 0.124982 |
| Distance per active animal-tick | 6.86 | 4.85 |
| Mean heading change | 25.27° | 11.90° |
| Heading changes over 90° | 7.98% | 2.99% |
| Heading changes over 150° | 2.64% | 0.96% |
| Births / deaths | 2,016 / 2,018 | 93 / 88 |
| Immigrants | 0 | 0 |

The runs ended with 34 and 67 animals respectively. Across all 300,000 ticks,
flat cost required 36 immigrants, versus 318 with energy-proportional cost.

## Conclusions and limits

Energy-proportional cost produced slower travel and fewer abrupt reversals in this
pair: late mean turn angle fell 53%, and the fraction of near reversals over 150°
fell 63%. This supports the smoothing hypothesis for this seed.

Both late populations harvested essentially the entire food-production limit of
0.125 energy per tick. The larger energy-proportional population therefore did not
produce more food; it sustained more, lower-energy animals with much less birth
and death turnover. Startup also needed more immigration. Larger population alone
does not establish better evolutionary performance.

This is one matched seed, not evidence of robustness across seeds. The two rules
produce different ecological and evolutionary trajectories; subsequent replication
should compare the same movement, resource, and reproductive measures.

Protocol, initial checkpoints, and aggregate results are under
`runs/2026-11-05-normalized-cost-comparison/`. The two run directories are
`runs/2026-11-05-normalized-movement-flat/` and
`runs/2026-11-05-normalized-movement-energy/`; each contains detailed metrics,
checkpoints, and `analysis/movement.json`. See [movement replay](../metrics.md)
for the reusable measurement tool.
