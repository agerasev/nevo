# Metrics

Eight inexpensive ecological metrics are enabled by default:

| Metric | Meaning |
| --- | --- |
| `population` | Animal count at the window endpoint. |
| `mean_animal_energy` | Mean energy of animals present at the endpoint. |
| `mean_plant_energy` | Mean energy of plants present at the endpoint. |
| `food_per_tick` | Total assimilated food energy divided by observed ticks. |
| `births_per_tick` | Births divided by observed ticks. |
| `deaths_per_tick` | Deaths divided by observed ticks. |
| `immigrants_per_tick` | Immigrants divided by observed ticks. |
| `distance_per_animal_tick` | Controlled travel divided by active animal-ticks. |

These describe abundance, energy availability, food collection, turnover, reliance
on immigration, and movement. They do not establish adaptation; that requires
controlled comparisons. Plant count is fixed by configuration. Neuron monitoring,
genetic reducers, per-survivor totals, truncated survival observations, and the
selectable count/sum/min/max/variance framework have been removed.

## Windows and configuration

Only collection and recording interval are configurable:

```toml
[ecosystem.metrics]
enabled = true
window_ticks = 1000
```

The [metrics preset](../configs/metrics.toml) uses 10,000 ticks. Windows are consecutive
intervals `(start_tick, end_tick]`. Each record includes actual tick count
(`samples`), active animal-ticks, and whether the window is complete. This metadata
is excluded from plotted series. One sample is written per complete window, plus
the unfinished window on orderly exit; the last full window is never duplicated.

Rates use actual exposure, including partial windows. An active animal-tick means
one animal sensed and acted. Newborns and immigrants first contribute next tick;
animals that later die still contribute their movement and events. Distance excludes
birth offsets and growth clamping. Empty populations have no energy mean; zero
active exposure has no distance rate. Unavailable values are omitted.

Changing the interval changes time resolution and smoothing, not units or scale.
A long-window rate equals the exposure-weighted average of its constituent windows:
use tick counts for event/food rates and active animal-ticks for distance. Snapshot
values at the same endpoint are identical regardless of interval. Different
endpoints can naturally differ; averages of endpoint snapshots are not time averages.
Resizing the viewer only changes downsampling, never recorded values.

## Files and resume

Each metrics-enabled invocation creates a new numbered file:

```text
runs/RUN/metrics/
  00000000000000000001.cborseq
  00000000000000000002.cborseq
```

Each file is a CBOR Sequence: one metadata record followed by window samples.
Metadata includes start tick, UTC timestamp, effective TOML configuration,
compiled Git revision, package version, and model identifiers. The local session ID
is 1; file boundaries distinguish invocations. There are no separator records between
invocations within new files. A zero-tick invocation writes metadata only.
The envelope format remains version 1; metric names describe their units.

Earlier files are never modified or scanned by the new writer. Orderly shutdown
syncs the current file. An interrupted trailing record remains in its original
file; readers can continue with the next invocation. Complete malformed records
are reported as errors, not silently skipped. Records are limited to 16 MiB.
There is no checksum or recovery past internal corruption.

The history reader also accepts the earlier `metrics.cborseq` at the run root,
read-only, before the new files. That format used metadata records and session IDs
to separate invocations, rather than raw separator bytes. Old records retain their
original metric names and definitions. Saved version-11 configurations discard
removed diagnostic selectors when resuming; explicit new configuration overlays
reject obsolete keys. Simulation state and checkpoint version are unchanged.

## Viewing

Select **Metrics** in the viewer. Search/select a metric, drag or scroll to pan,
Ctrl-scroll or pinch to zoom, and hover for values and window bounds. **Follow data**
fits arriving records; panning or zooming disables it. **Fit all** restores the full
view. Diamonds mark partial windows. Missing values and invocation boundaries break
curves, including overlapping ticks after restoring an older checkpoint.

The viewer loads history once, discovers new files, and follows the current file
even while **World** is selected. It reads bounded batches outside Universe locks,
polling every 200 ms when caught up. Original samples stay in memory; plotting does
not reread them. Display downsampling retains extrema and endpoints, so narrow plots
preserve spikes. These extrema are drawing choices, not additional metrics.
Memory grows with recorded history. `viewer.tab = "metrics"` selects this tab initially.

## Collection

[Metrics](../src/metrics/mod.rs) is a read-only typed-summary contract. The ecosystem
collects event and movement rates from existing `TickReport` data; energy means
read current bodies/plants only when a summary is requested. No monitoring caches
or selectors live in bodies, plants, minds, or genomes. Neither `Mind` nor
`Simulation` requires the metrics trait.

The ecosystem's small window accumulator is part of the prepared tick update:
failed or discarded updates publish nothing. It consumes no randomness and is
excluded from serialized simulation state. Resume starts a fresh window at the
restored tick. Before any tick, population and energy snapshots are available but
rates are not. Previous logs never reconstruct simulation state or accumulators.

For a targeted movement investigation, the separate
[`movement_report` example](../examples/movement_report.rs) replays a saved checkpoint
with the same build, verifies its complete final state, and writes
`analysis/movement.json`. It measures heading changes and energy budgets offline;
these additional diagnostics are not part of normal runtime monitoring.
