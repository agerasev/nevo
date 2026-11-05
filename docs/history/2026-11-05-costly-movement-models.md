# Controllers after 8.85 million ticks with costly movement

**November 5, 2026.** Analyze run `runs/2026-11-05_20-50-31`, seed
`15205546245516027036`, final tick 8,846,674. Twenty 3–4–3 RNNs remain.
Movement costs `0.003 × pre-movement energy × distance`, with upkeep 0.0005,
500 plants, and immigration floor 20. Metadata records `ffe13de`, then two
`ffe13de-dirty` invocations; physical configuration is unchanged across them.

## Controller responses

All final controllers still primarily follow food gradients, at substantially
lower speed than the [earlier controllers](2026-11-05-long-run.md). Across 360
constant gradient directions at the final world's median food strength (0.516),
99.71% of moving settled responses point up-gradient, with mean heading error
25.0°. Median commanded speed is 71.0% of maximum; the earlier population gives
98.3% under the same synthetic inputs. These populations evolved under different
conditions, so this is not a controlled estimate of movement-cost effects.

Two separated genetic groups contain 14 and 6 animals, with mean swept speed
72.9% and 59.5%. Their representatives are 3668 and 3579. Raising food strength
from the observed minimum 0.419 to maximum 0.683 increases commanded speed in
every tested genome/direction combination, by 3.47 percentage points on average.
Speed also depends strongly on gradient direction; there is no energy input.

Recurrence matters to steering: clearing memory each tick raises mean heading
error from 25.0° to 57.0°. Gradient reversals usually settle within three updates,
with tested worst cases of six. Diverse initial memories converge to one fixed
state under every tested constant input; no sustained oscillation appeared.

## Ecological context and verification

The final 16,674 ticks replayed from the tick-8,830,000 checkpoint to exactly the
saved final state. Mean distance was 6.758 per active animal-tick, mean turn angle
20.8°, and 5.48% of consecutive moving headings changed by over 90°. Food intake
was 0.123764 per tick, 99.0% of the production ceiling. Movement accounted for
91.65% of energy expenditure. This interval had one birth, two deaths, and no
immigrants.

Across the full run there were 3,327 births and 398 immigrants. In the final
849,235 recorded ticks there were 81 births, 93 deaths, and 10 immigrants; the
run ended at its population floor. Long elapsed simulation time therefore does
not imply equally extensive reproductive turnover or robust independence from
immigration.

The evidence supports slower, food-dependent gradient following with short
recurrent adjustment. The [earlier heuristic comparison](2026-11-05-gradient-comparison.md)
used different movement costs and does not establish whether these controllers
outperform a maximum-speed gradient follower here.

The run's `analysis/` directory contains `controller-probe.py`, numerical results,
representative weight and behavior figures, `movement.json`, and detailed methods
in `controller-interpretation.md`. NumPy probes agreed with Rust f32 calculations
within 2.25e-7 over 1,600 updates. The whole metrics log passed continuity and
population-accounting checks across its three invocation boundaries.
