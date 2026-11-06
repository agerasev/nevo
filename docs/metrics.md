# Metrics

Keep measurements inexpensive and tied to an experimental question. Runtime
monitoring describes abundance, food collection, turnover, immigration, and
movement; it does not by itself establish adaptation.

| Metric | Interpretation |
| --- | --- |
| `population` | Animals at the window endpoint. |
| `mean_animal_energy`, `mean_plant_energy` | Energy means at that endpoint. |
| `food_per_tick` | Assimilated energy per observed tick. |
| `births_per_tick`, `deaths_per_tick`, `immigrants_per_tick` | Events per observed tick. |
| `distance_per_animal_tick` | Controlled travel per active animal-tick. |

Set `ecosystem.metrics.enabled` and `window_ticks`, or layer the
[metrics preset](../configs/metrics.toml). Windows cover `(start_tick, end_tick]`;
orderly exit records any partial window. Rates use actual exposure, so changing
window length changes smoothing, not units or scale. Combine event/food rates
with tick weights and movement rates with active-animal-tick weights.

An active animal-tick means an animal sensed and acted, including animals that
later die. New arrivals contribute when they first act. Distance excludes birth
offsets and growth clamping. Undefined means/rates are absent. Endpoint snapshots
at different ticks can differ; averaging them does not produce a time average.
Resizing the viewer changes plotting resolution, never recorded values.

Each enabled invocation creates a separate numbered CBOR Sequence under `metrics/`,
containing metadata then samples. Earlier files remain untouched. An incomplete
trailing record does not prevent following the next file; complete malformed
records are errors. The older root `metrics.cborseq` is supported read-only, with
its original definitions and session boundaries. Logs never reconstruct state.

Collection is observationally pure. Accumulators publish with their tick and restart
on resume; population and energy snapshots are available before the first tick,
but rates are not. See [Metrics](../src/metrics/mod.rs),
[ecosystem summaries](../src/ecosystem/metrics.rs), and [log format](../src/metrics/log/mod.rs).

## Viewing

In **Metrics**, search/select a series, drag or scroll to pan, Ctrl-scroll or pinch
to zoom, and hover for values. **Follow data** fits arriving records; **Fit all**
restores the full view. Diamonds mark partial windows; missing values and invocation
boundaries break curves. History stays in memory, so memory use grows with it.

For targeted movement investigations, [movement_report](../examples/movement_report.rs)
replays checkpoints and checks the complete final state before writing offline
heading and energy diagnostics. These are not part of runtime monitoring.
