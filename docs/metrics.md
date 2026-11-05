# Metrics

Layer the [metrics preset](../configs/metrics.toml) over an experiment configuration:

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/metrics.toml --seed 1 --ticks 250
cargo run --locked -- runs/my-run --config configs/metrics.toml --ticks 250
```

Metrics default off. `[ecosystem.metrics] enabled = true` enables observation windows
and output; component selectors choose their contents. `window_ticks` defaults to
100 and must be positive when enabled. Windows are consecutive, non-overlapping
tick intervals `(start_tick, end_tick]`. Each summary gives its actual bounds,
completed tick count (`samples`), and whether the window is complete.

Each run has one append-only `metrics.cborseq`, a CBOR Sequence of independently
encoded records. Each metrics-enabled invocation appends immutable metadata: its
ID, start/resume tick, UTC timestamp, effective TOML config, compiled Git revision
(with a dirty marker when applicable), package version, and model identifiers.
A source archive without Git records an unknown revision. Window samples reference
the invocation ID and carry their end tick and finalized component summary.

One sample is written per completed window, plus the unfinished window on orderly
exit. Finishing exactly at a boundary does not duplicate that window. A zero-tick
invocation records metadata only. Snapshot values describe the end of the window;
event totals cover all its ticks. Previous invocations' configuration stays in the
file even when `config.toml` changes. The current metrics format is version 1;
old JSONL logs are not imported or modified.

The [log reader](../src/metrics/log/mod.rs) always follows the same file: it reads
existing records, waits at EOF or an incomplete tail, then accepts later appends.
It also accepts creation after the reader starts. It never takes the run writer
lock. Polls bound the work per call; complete records before a corrupt item remain
readable, and the next poll reports that item as an error.

Before appending, the writer validates the entire file under the run writer lock.
It removes only an incomplete final CBOR item. Malformed CBOR, invalid schemas,
and invalid record ordering produce an error without changing the file. Records
are limited to 16 MiB. Each append becomes visible to readers; orderly shutdown
also syncs the file to disk. Power loss can still lose unsynced data. CBOR Sequence
has no checksum and cannot reliably recover past an internally corrupted record.

## Viewing

Attach the viewer and select **Metrics** to browse the recorded scalar series:

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/metrics.toml --viewer
```

Use the search field and metric selector, drag or scroll to pan, Ctrl-scroll or
pinch to zoom, and hover for values and window bounds. **Follow data** fits new
records automatically; panning or zooming turns it off. **Fit all** resets the view.
Diamonds mark partial windows. Missing values break curves; separate invocation curves preserve resume
boundaries and overlapping ticks after restoring an older checkpoint.

The viewer loads the full plotting history into memory once and adds new windows
as they arrive. Panning, zooming, and selecting metrics use these retained values
without rereading the file. A background worker loads records and downsamples
plots, keeping that work outside the UI and Universe locks. Zoomed-out curves
retain the first, last, minimum, and maximum recorded values in horizontal display
bins; zooming in reveals the original points. There is no history cap, offset
index, or additional file. Memory use grows with the number of windows and metrics;
this is the current simplicity tradeoff.

The reader follows appends even while World is selected and after catching up;
there is no historical/live mode distinction. Browsing an older range keeps that
viewport when new windows arrive. Loading and plotting use the log, without reading
or advancing the Universe. Collection continues independently of the selected tab.
Switching to Metrics skips world scene collection and drawing in the normal viewer.
`viewer.tab = "metrics"` selects it initially (`"world"` is the default).

## Selection and interpretation

Scalar selections accept `count`, `sum`, `mean`, `min`, `max`, and `variance` booleans;
each defaults to false. For example:

```toml
[ecosystem.metrics]
enabled = true
window_ticks = 100
population = true
event_rates = true
genetic_diversity = true

[ecosystem.animal.body.metrics.energy]
count = true
mean = true
min = true
max = true

[ecosystem.animal.metrics.offspring]
sum = true
mean = true
max = true
```

Selections belong to the component which defines the measurement. The typed
configuration is borrowed down the same ownership tree used for computation.
Configuration files, including these selections, are saved with the run; selectors
and window length can change on resume.

| Selection | Measurement |
| --- | --- |
| `ecosystem.metrics.population` | Current animal and plant counts. |
| `ecosystem.metrics.event_rates` | Meals, food gained, births, deaths, and immigrants over the window; birth/death/immigration rates per observed tick and food/meal/birth/death rates per active animal-tick. |
| `ecosystem.animal.metrics.food_gained` | Window food gain per currently present animal. |
| `ecosystem.animal.metrics.offspring` | Window offspring count per currently present animal. |
| `ecosystem.animal.metrics.distance` | Window controlled movement distance per currently present animal, excluding birth offsets. |
| `ecosystem.animal.metrics.observed_survival` | Active ticks observed within the window for animals that died in it. |
| `ecosystem.animal.body.metrics.energy` | Current animal energy distribution. |
| `ecosystem.plant.metrics.energy` | Current plant energy distribution. |
| `ecosystem.animal.mind.brain.metrics.activation` | Latest hidden activations across current brains that have acted since initialization or resume. |
| `ecosystem.animal.mind.brain.metrics.saturation` | Fraction of those hidden units with absolute activation at least 0.95. |
| `ecosystem.metrics.genetic_diversity` | Mean population variance across corresponding genome coordinates, including newborns. |

An active animal-tick means an animal sensed and acted during that tick; newborns
and immigrants first contribute on the next tick. Rates use actual exposure, even
in partial windows. Zero exposure makes the corresponding rate unavailable.
Selected empty counts and sums are zero; empty means, extrema, and variances are
unavailable and omitted from recorded summaries. Variance is population variance, `M2 / n`.

Window offspring and food gain are useful reproductive and foraging proxies.
They are not proof of adaptation: immigration can sustain an unfit population,
resource density changes opportunity, and summaries over surviving animals omit
animals that died. World event totals retain those animals' events. Observed
survival is truncated by the window and includes only deaths, so it is not a
lifespan estimate. Brain activity and genetic variance are diagnostics, not fitness.
Controlled comparisons across seeds and conditions remain necessary.

## Contracts and caches

[Metrics](../src/metrics/mod.rs) exposes a typed `Summary` through
`collect_metrics(&self, &mut summary)` and the convenience `metrics()` query.
Bodies, plants, minds, genomes, and animals merge into typed summaries. The
ecosystem replaces its complete snapshot on collection: pooling different worlds
or windows requires an explicit analysis operation. There is no runtime registry.
Genome diversity is computed on demand over matching weight coordinates; variance among different weights within one genome is not diversity.

[Aggregate](../src/metrics/aggregate.rs) provides `push`, `merge`, and `value`.
Count, sum, mean, min, max, and Welford moments can combine independently collected
samples. `Selection` and `SelectedStats` update only requested reductions. A merge
combines each reduction's enabled samples; use matching selections for a common
sample population.

Event counters live in skipped caches on the ecosystem and concrete animal bodies;
brain diagnostics live in the RNN. The generic `Animal<B, M>` still owns only body
and mind. Consequently animal event distributions appear under `animal.body` in
output, while their selection belongs to `animal.metrics`. Snapshot energy and
genome reductions run when queried; event caches update on every committed tick.

Computation creates candidate cache changes under the read lock; publication applies
them with the physical and mind updates under the write lock. Failed or discarded
updates publish no metrics. Collection itself is read-only and consumes no randomness.
Neither `Mind` nor `Simulation` requires the `Metrics` trait.

All accumulators are omitted from checkpoints. Resume starts the next window at the restored
tick and fills it from new observations. New records append to the same log;
previous records are never used to reconstruct accumulators. Recurrent
memory and RNG state remain checkpointed, so resetting metrics preserves dynamics.
