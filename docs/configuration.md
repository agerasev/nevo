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
give fixed, seeded Gaussian patches a local plant capacity. Patch centers persist
in checkpoints, independently of animal initialization. The optional
`plant.replenishment` table enables density-based replenishment; omitting it keeps
the baseline's immediate replacement to a fixed plant count.

Replenishment samples fresh continuous positions according to unused capacity.
Crowding sums a Gaussian contribution from every plant, including zero-energy
seedlings, with correction for arena boundaries. Its radius is independent of
patch spread. Plants retain room to mature inside the arena, and there is no grid
or fixed planting site. Capacity is a soft local limit: adding one plant raises
density throughout its neighborhood and can cause a local overshoot. Total plant
count varies with consumption and recovery.

The [replenishment contract](../src/ecosystem/replenishment/mod.rs) defines density
units, recovery, and sampling limits. Importance sampling concentrates proposals
near patches when useful, with acceptance adjusted to the proposal distribution.
Successful and rejected proposals, including draws outside the usable arena,
consume the tick budget; a full landscape need not admit any seedlings. Settings
that exceed the configured attempt limit are rejected rather than silently
slowing recovery. Initial planting uses the same crowding rule and must establish
the requested `ecosystem.plants` within its separate attempt limit or fail clearly.
Explicit initial positions override that sampling; fixed replacement positions
are incompatible with density-based replenishment.

The [patch-growth overlay](../configs/patch-growth.toml) independently varies
individual plant growth across the same landscape. The resource-patches preset
keeps per-plant growth uniform, but plant count and food production can still
change. Compare [measured plant production](metrics.md) as well as collected food.
For a bounded patchy-world run:

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
