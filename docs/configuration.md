# Configuration

The executable has one parameterized ecosystem; preset names have no meaning in
Rust code. [Baseline](../configs/baseline.toml) and [debug](../configs/debug.toml)
specify ecosystem parameters. [Viewer](../configs/viewer.toml) attaches the window
with VSync; it does not pace simulation ticks. Presets leave the run duration unlimited.

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/viewer.toml
cargo run --locked -- --config configs/debug.toml --ticks 32
cargo run --locked -- --config configs/baseline.toml --viewer --tick-hz 25 --seed 42
cargo run --locked -- runs/my-run --config configs/viewer.toml
```

Precedence, from lowest to highest: defaults; saved run configuration (or checkpoint
configuration when its TOML file is missing); `--config` files in command-line
order; explicit CLI values. Tables merge recursively. Scalars and arrays replace
the previous value. Each file is checked for unknown keys and invalid types, even
when later files override them. The resolved configuration is saved, not the file
list, so resuming does not depend on the original presets.

The [Serde config types](../src/app/config/mod.rs) define the full schema and its
defaults. The independent [clap CLI](../src/app/cli.rs) exposes only common shortcuts;
config fields do not automatically become flags. `--help` lists the shortcuts without
loading files. Quote seeds larger than TOML's signed integer range.

| CLI shortcut | Config field |
| --- | --- |
| `--seed N` | `seed` |
| `--ticks N` | `ticks` |
| `--tick-hz N` | `tick_hz` |
| `--viewer[=true\|false]` | `viewer.enable` |
| `--paused[=true\|false]` | `paused` |

Bare boolean shortcuts mean true; `--viewer=false` and `--paused=false` override true
values. Omitted shortcuts preserve file settings. The positional run directory,
[`--latest`](persistence.md), and repeated `--config` paths select inputs and are
not persisted. There is no `--hz` alias, generated nested flags, or `--set` interface.

Viewer configuration is a table (equivalently, dotted TOML keys):

```toml
[viewer]
enable = true
vsync = true
tab = "world" # or "metrics"
```

`enable` defaults to false, `vsync` to true, and `tab` to `"world"`.
The [Metrics tab](metrics.md#viewing) follows the run's recorded statistics. `tick_hz` defaults to zero, meaning
unlimited simulation throughput. A positive value limits simulation ticks per second;
it does not change the simulated timestep or frame rate. Only `viewer.enable` has a
CLI shortcut; use config files for VSync and all ecosystem details.

All paths below are relative to `[ecosystem]`. Each run's resolved `config.toml`
includes these defaults, so it also serves as a complete editable configuration.

| Setting | Default | Meaning |
| --- | --- | --- |
| `initial_animals` | `20` | Number admitted at startup. |
| `plants` | `500` | Maintained plant count. |
| `minimum_population` | `20` | Immigration floor; zero disables automatic immigration. |
| `world.width`, `world.height` | `1260`, `700` | Centered arena dimensions. |
| `immigration.energy` | `100` | Energy for every admitted animal. |
| `immigration.position` | `"random"` | Placement for every admitted animal. |
| `animal.body.upkeep` | `2` | Energy spent per animal per tick. |
| `animal.body.movement_cost_per_distance` | `0` | Energy per unit of controlled distance after boundary clamping. |
| `animal.body.max_speed`, `animal.body.movement_time_step` | `100`, `0.1` | Movement scale; maximum displacement is their product. |
| `animal.body.movement_dead_zone` | `0.01` | Direction magnitudes at or below this do not move. |
| `animal.body.radius_scale` | `0.5` | Radius coefficient in `scale × sqrt(max(energy, 0))`. |
| `animal.reproduction.threshold` | `400` | Reproduction requires energy strictly above this. |
| `animal.reproduction.child_energy_fraction` | `0.5` | Fraction of pre-split energy transferred to the child. |
| `animal.reproduction.separation_scale` | `0.5` | Parent/child offset relative to the parent's post-split radius. |
| `animal.feeding.efficiency` | `0.1` | Fraction of eaten plant energy gained by the animal. |
| `animal.feeding.contact_scale` | `0.8` | Feeding distance relative to combined radii. |
| `plant.initial_energy`, `plant.replacement_energy` | `400`, `0` | Initial and replacement plant energy. |
| `plant.growth`, `plant.max_energy` | `1`, `400` | Growth per tick and energy cap. |
| `plant.radius_scale` | `0.5` | Plant radius coefficient, using the same energy/radius equation. |
| `plant.initial_position`, `plant.replacement_position` | `"random"`, `"random"` | Initial and replacement placement. |
| `sensing.distance_scale` | `0.5` | Displacements are multiplied by this divided by arena half-height. |
| `sensing.softening` | `0.01` | Positive distance offset in the food field. |
| `sensing.gradient_dead_zone` | `0.01` | Smaller/equal gradient magnitudes become zero; others normalize. |
| `genetics.initialization.hidden_units` | `4` | Recurrent brain width; inputs and outputs remain three-dimensional. |
| `genetics.initialization.weight_min`, `genetics.initialization.weight_max` | `-0.5`, `0.5` | Uniform interval `[min, max)`; equal bounds produce a constant without random draws. |
| `genetics.mutation.max_delta` | `0.02` | Uniform mutation noise in `[-max_delta, max_delta)` per parameter. |

Initial and later immigrants use exactly the same admission settings and path;
only the number requested differs. Placement accepts `"random"` or `[x, y]`.
An explicit `"random"` overrides a fixed position from an earlier file. Fixed
positions must be finite and inside the configured arena; bodies remain clamped
to its bounds. Startup admits animals before seeding plants, preserving draw order.

The [movement-cost preset](../configs/movement-cost.toml) sets upkeep to `0.2`
and movement cost to `0.18` per distance unit. At the default maximum displacement
of 10, an acting animal spends between 0.2 and 2 energy per tick. Blocked movement
costs no movement energy; birth offsets and growth clamping are excluded. Movement
fees are deducted after feeding and before reproduction; all present animals pay
upkeep at the end of the tick. This is an experimental setting, with reduced jitter
still an untested hypothesis.

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/movement-cost.toml --config configs/metrics.toml --config configs/viewer.toml
```

`movement_time_step` scales displacement and hence distance-based movement cost:
upkeep and plant growth remain per-tick quantities. `tick_hz` changes wall-clock
pacing only. Boundary clamping, the
food-field equations, hidden `tanh`, and observation/action meanings remain model
rules. Hidden width is fixed within a run; evolution changes weights and biases.

`trace_ticks` prints decisions and events through an absolute tick; zero disables
tracing. Tracing, checkpoint policy, and `smoke` are config-only settings.
The [smoke preset](../configs/smoke.toml) enables the viewer's twelve-frame check.
Runtime options such as viewing,
pacing, tracing, tick budget, checkpoint policy, and metric selections can change
on resume. Seed and all physical, genetic, and initialization settings must match
the checkpoint; conflicting overrides fail before replacing saved files. See
[persistence](persistence.md) for format and recovery.

Metrics configuration follows component ownership: `ecosystem.metrics`,
`ecosystem.animal.metrics`, `ecosystem.animal.body.metrics`,
`ecosystem.animal.mind.brain.metrics`, and `ecosystem.plant.metrics`.
The [detailed metrics preset](../configs/metrics.toml) is an overlay; see
[selection syntax and window semantics](metrics.md).
