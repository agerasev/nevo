# Configuration

Presets compose one parameterized experiment; their filenames have no meaning in
Rust. The [Serde types](../src/app/config/mod.rs) and
[ecosystem settings](../src/ecosystem/config.rs) define the schema and defaults.
The separate [CLI](../src/app/cli.rs) exposes common shortcuts, not every field.
Each run's resolved `config.toml` is a complete editable configuration.

Precedence is defaults, saved configuration, ordered `--config` files, then
explicit CLI values. Tables merge by field; scalars and arrays replace. Every
file is checked even if later files override it. Preset paths are not needed to
resume. Quote seeds beyond TOML's signed integer range.

[Baseline](../configs/baseline.toml) and [debug](../configs/debug.toml) provide
starting populations. Layer [viewer](../configs/viewer.toml) and
[metrics](../configs/metrics.toml) settings as needed. Choose a complete energy
budget with [upkeep only](../configs/movement/upkeep-only.toml),
[flat movement cost](../configs/movement/flat.toml), or
[energy-proportional movement cost](../configs/movement/cost.toml).

The bounded [larger-world preset](../configs/large-world.toml) expands the arena
while preserving initial plant and animal densities. Local sensing uses fixed
`sensing.distance_scale` and `sensing.signal_scale`, independent of arena dimensions
and plant count. Their defaults retain the original 500-plant baseline's signal
scale; sparse worlds no longer amplify each source by averaging over fewer plants.
Sources still contribute at all distances; finite-range sensing is a later milestone.
This changes the world model, so earlier checkpoints require their original build.

Layer [resource patches](../configs/resource-patches.toml) on the larger world to
cluster plants around fixed, seeded Gaussian patches. Both initial and replacement
plants sample the same landscape; harvesting can still change the standing
distribution. Patch centers persist in checkpoints, independently of animal
initialization. Explicit plant positions override random placement.

`plant.patches.density_fraction` mixes clustered and uniform placement. The optional
[patch-growth overlay](../configs/patch-growth.toml) independently varies growth
between a floor in poor areas and a peak at patch centers. Overlapping patches
blend smoothly without exceeding the peak. See the [patch contract](../src/ecosystem/patches/mod.rs)
for settings and sampling bounds. Plants need room to mature inside the arena;
Gaussian samples are truncated, not clamped onto its edges.

The density-only preset keeps the original per-plant growth rate. When varying
growth, compare [measured plant production](metrics.md) as well as collected food:
clustering plants in fertile regions can increase supply even with unchanged
plant count. Patches are disabled by default. For a bounded patchy-world run:

```sh
cargo run --locked -- --config configs/large-world.toml --config configs/resource-patches.toml --seed 1
```

Energy units use a reference reproduction threshold of `1.0`, consistently for
animals, plants, food gains, and costs. Changing the configured threshold changes
the experiment, not its units. Earlier logs keep their original units.
Spatial units are unchanged. Movement time scales displacement; upkeep and plant
growth remain per-tick quantities. `tick_hz` changes wall-clock pacing only.

Seed and model settings are immutable on [resume](persistence.md). Runtime
settings such as viewing, pacing, tracing, tick budget, checkpoint policy, and
metrics may change.
