# Configuration

The executable has one parameterized ecosystem; preset names have no meaning in
Rust code. [Baseline](../configs/baseline.toml) and [debug](../configs/debug.toml)
specify ecosystem parameters. [Viewer](../configs/viewer.toml) attaches the window
and paces ticks at 25 Hz. Presets leave the run duration unlimited.

```sh
cargo run --locked -- --config configs/baseline.toml --config configs/viewer.toml
cargo run --locked -- --config configs/debug.toml --ticks 32
cargo run --locked -- --config configs/baseline.toml --ecosystem-plants 1000 --seed 42
cargo run --locked -- runs/my-run --config configs/viewer.toml
```

Precedence, from lowest to highest: defaults; saved run configuration (or checkpoint
configuration when its TOML file is missing); `--config` files in command-line
order; explicit CLI values. Tables merge recursively. Scalars and arrays replace
the previous value. Each file is checked for unknown keys and invalid types, even
when later files override them. The resolved configuration is saved, not the file
list, so resuming does not depend on the original presets.

Run options use snake_case in TOML and kebab-case on the CLI. Model options live in
`[ecosystem]` and have an `--ecosystem-` CLI prefix. Booleans accept bare flags or
explicit values such as `--viewer=false`. Quote seeds larger than TOML's signed
integer range. `--help` lists all settings and defaults without loading files.

| Ecosystem setting | Meaning |
| --- | --- |
| `initial_animals` | Animals admitted at startup. |
| `plants` | Plant count, maintained by replacement after feeding. |
| `minimum_population` | Population floor maintained by immigration; zero disables it. |
| `initial_animal_energy` | Energy of initial animals; later immigrants use the model default. |
| `initial_animal_position` | Initial animal placement: `"random"` or `[x, y]`. |
| `initial_plant_position` | Initial plant placement; replacements remain random. |

Positions accept `random` or `X,Y` on the CLI. An explicit `"random"` overrides a
fixed position from an earlier file. Fixed positions must be finite and inside the
arena; animals are clamped to keep their bodies inside. Counts, energy, and the two
placements are independent. Initialization still admits animals before seeding
plants, preserving the random draw order.

`trace_ticks` prints decisions and events through an absolute tick; zero disables
tracing. It is independent of ecosystem settings. Runtime options such as viewing,
pacing, tracing, tick budget, and checkpoint policy can change on resume. Seed and
ecosystem settings must match the checkpoint; conflicting overrides fail before
replacing saved files. See [persistence](persistence.md) for format and recovery.
