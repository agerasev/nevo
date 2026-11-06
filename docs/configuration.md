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
| `immigration.energy` | `0.25` | Energy for every admitted animal. |
| `immigration.position` | `"random"` | Placement for every admitted animal. |
| `animal.body.upkeep` | `0.005` | Energy spent per animal per tick. |
| `animal.body.movement_cost` | `0` | Coefficient in `cost × distance × energy^power`. |
| `animal.body.movement_energy_power` | `0` | Nonnegative exponent of pre-movement energy. |
| `animal.body.max_speed`, `animal.body.movement_time_step` | `100`, `0.1` | Movement scale; maximum displacement is their product. |
| `animal.body.movement_dead_zone` | `0.01` | Direction magnitudes at or below this do not move. |
| `animal.body.radius_scale` | `10` | Radius coefficient in `scale × sqrt(max(energy, 0))`. |
| `animal.reproduction.threshold` | `1` | Reproduction requires energy strictly above this. |
| `animal.reproduction.child_energy_fraction` | `0.5` | Fraction of pre-split energy transferred to the child. |
| `animal.reproduction.separation_scale` | `0.5` | Parent/child offset relative to the parent's post-split radius. |
| `animal.feeding.efficiency` | `0.1` | Fraction of eaten plant energy gained by the animal. |
| `animal.feeding.contact_scale` | `0.8` | Feeding distance relative to combined radii. |
| `plant.initial_energy`, `plant.replacement_energy` | `1`, `0` | Initial and replacement plant energy. |
| `plant.growth`, `plant.max_energy` | `0.0025`, `1` | Growth per tick and energy cap. |
| `plant.radius_scale` | `10` | Plant radius coefficient, using the same energy/radius equation. |
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

Energy is expressed in units where the default reproduction threshold is `1.0`.
An immigrant starts at `0.25`; an ordinary newborn receives about `0.5`. This
convention applies to animal and plant energy, food gains, upkeep, and metrics.
Changing the reproduction threshold changes the experiment, not its units.
Spatial units and tick timing are unchanged. Earlier run logs retain their recorded
units; values are not automatically converted when reading them.

Movement costs `movement_cost × distance × energy_before_movement^movement_energy_power`.
Power zero gives a multiplier of one even at zero energy; positive powers give
zero at zero energy. A zero coefficient disables the fee. Costs use actual
controlled distance after boundary clamping, excluding birth offsets and growth
clamping. They are deducted after feeding and before reproduction, using the
energy measured before movement and feeding. All present animals then pay upkeep.

Compose one of these cost presets with the baseline. Each sets all three fields,
so the last cost preset wins without retaining part of an earlier selection.

| Preset | Upkeep | Movement coefficient | Energy power |
| --- | ---: | ---: | ---: |
| [Upkeep only](../configs/movement/upkeep-only.toml), also the default | 0.005 | 0 | 0 |
| [Flat movement](../configs/movement/flat.toml) | 0.0005 | 0.00045 | 0 |
| [Energy-proportional movement](../configs/movement/cost.toml) | 0.0005 | 0.003 | 1 |

At energy `0.5` and maximum displacement `10`, the flat preset costs `0.005`
and the energy-proportional preset `0.0155` per tick, including upkeep. Other nonnegative powers are also configurable.

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/movement/cost.toml --config configs/metrics.toml --config configs/viewer.toml
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
