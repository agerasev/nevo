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

| Ecosystem setting | Meaning |
| --- | --- |
| `initial_animals` | Animals admitted at startup. |
| `plants` | Plant count, maintained by replacement after feeding. |
| `minimum_population` | Population floor maintained by immigration; zero disables it. |
| `initial_animal_energy` | Energy of initial animals; later immigrants use the model default. |
| `initial_animal_position` | Initial animal placement: `"random"` or `[x, y]`. |
| `initial_plant_position` | Initial plant placement; replacements remain random. |

An explicit `"random"` overrides a
fixed position from an earlier file. Fixed positions must be finite and inside the
arena; animals are clamped to keep their bodies inside. Counts, energy, and the two
placements are independent. Initialization still admits animals before seeding
plants, preserving the random draw order.

`trace_ticks` prints decisions and events through an absolute tick; zero disables
tracing. Tracing, checkpoint policy, and `smoke` are config-only settings.
The [smoke preset](../configs/smoke.toml) enables the viewer's twelve-frame check.
Runtime options such as viewing,
pacing, tracing, tick budget, checkpoint policy, and metric selections can change
on resume. Seed and ecosystem initialization settings must match the checkpoint; conflicting overrides fail before
replacing saved files. See [persistence](persistence.md) for format and recovery.

Metrics configuration follows component ownership: `ecosystem.metrics`,
`ecosystem.animal.metrics`, `ecosystem.animal.body.metrics`,
`ecosystem.animal.mind.brain.metrics`, and `ecosystem.plant.metrics`.
The [detailed metrics preset](../configs/metrics.toml) is an overlay; see
[selection syntax and window semantics](metrics.md).
