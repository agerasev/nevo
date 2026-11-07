# Configuration

Presets compose one parameterized experiment; their filenames have no meaning in
Rust. The [Serde types](../src/app/config/mod.rs) and
[ecosystem settings](../src/ecosystem/config.rs) define the schema and defaults.
The separate [CLI](../src/app/cli.rs) exposes common shortcuts, not every field.
Each run's resolved `config.toml` is a complete editable configuration.
Reusable presets leave run duration unset; use `--ticks N` for a bounded run.

Precedence is defaults, saved configuration, ordered `--config` files, then
explicit CLI values. Tables merge by field; scalars and arrays replace. Every
file is checked even if later files override it. Preset paths are not needed to
resume. Quote seeds beyond TOML's signed integer range.

Choose one file from each relevant directory and combine them with repeated
`--config` arguments. Omitted choices retain defaults or saved values. World
presets own the arena, initial populations, and resource landscape; other
directories configure separate aspects of the experiment.

| Choice | Presets |
| --- | --- |
| World | [Small uniform](../configs/world/small-uniform.toml), [small non-uniform](../configs/world/small-nonuniform.toml), [large uniform](../configs/world/large-uniform.toml), [large non-uniform](../configs/world/large-nonuniform.toml), [debug](../configs/world/debug.toml) |
| Obstacles | [None](../configs/obstacles/none.toml), [32 random circles](../configs/obstacles/scattered.toml), [16 random circles](../configs/obstacles/scattered-small.toml) |
| Brain | [Small: 4 hidden units](../configs/brain/small.toml), [Large: 16 hidden units](../configs/brain/large.toml) |
| Mutation | [Adaptive scale](../configs/mutation/adaptive.toml), [fixed scale](../configs/mutation/fixed.toml), [disabled](../configs/mutation/none.toml) |
| Locomotion | [Damped acceleration](../configs/locomotion/damped.toml) |
| Movement energy budget | [Work and motor losses](../configs/movement/work-and-loss.toml), [upkeep only](../configs/movement/upkeep-only.toml) |
| Plant growth | [Uniform](../configs/plant-growth/uniform.toml), [patch-dependent](../configs/plant-growth/patches.toml) |
| Viewer | [Window](../configs/viewer/window.toml) |
| Metrics | [Long window](../configs/metrics/long-window.toml) |
| Diagnostics | [Tick trace](../configs/diagnostics/trace.toml) |

Mutation presets configure [RNN genetics](../src/mind/rnn/genome.rs).
`initialization.mutation_sigma` sets the scale for randomly initialized animals;
offspring inherit their parent's scale, and immigrant templates retain their own.
`mutation.log_sigma_std` controls scale adaptation; zero freezes the scale while
weights and biases still mutate. `mutation.enabled = false` disables all mutation
without consuming randomness. The initial default scale matches the old uniform
noise variance; changing the distribution or adapting its scale does not establish
an evolutionary advantage.

Large-world presets use a sparser plant population across the larger arena.
Local sensing uses fixed `sensing.distance_scale` and
`sensing.signal_scale`, independent of arena dimensions
and plant count. Their defaults retain the original 500-plant baseline's signal
scale; sparse worlds no longer amplify each source by averaging over fewer plants.
Sources still contribute at all distances and through obstacles; finite-range
sensing and occlusion are later work.
This changes the world model, so earlier checkpoints require their original build.

Non-uniform worlds give fixed, seeded Gaussian patches a local plant capacity.
Patch centers are sampled uniformly across the usable arena and persist in
checkpoints, independently of animal initialization and brain size.
The optional `plant.replenishment` table enables density-based replenishment; omitting it keeps
the baseline's immediate replacement to a fixed plant count.

Replenishment samples fresh continuous positions according to unused capacity.
Crowding sums a Gaussian contribution from every plant, including zero-energy
seedlings, with correction for arena boundaries. Its radius is independent of
patch spread. Proposed sites leave room to mature inside the arena, and there is no grid
or fixed planting site. Capacity is a soft local limit: adding one plant raises
density throughout its neighborhood and can cause a local overshoot. Total plant
count varies with consumption and recovery.

The [replenishment contract](../src/ecosystem/replenishment/mod.rs) defines density
units, recovery, and sampling limits. Importance sampling concentrates proposals
near patches when useful, with acceptance adjusted to the proposal distribution.
Successful and rejected proposals, including draws outside the usable arena or
inside obstacles, consume the tick budget; a full landscape need not admit any
seedlings. Settings
that exceed the configured attempt limit are rejected rather than silently
slowing recovery. Initial planting uses the same crowding rule and must establish
the requested `ecosystem.plants` within its separate attempt limit or fail clearly.
Explicit initial positions override that sampling; fixed replacement positions
are incompatible with density-based replenishment.

Obstacle presets sample radii and centers uniformly from the run seed, keeping
each circle inside the arena. Their random stream is independent of the plant
patches and brain initialization. The generated circles persist in checkpoints;
resuming does not resample them. Explicit obstacle arrays are also supported.
Admission avoids obstacles with bounded proposals; explicit positions must also
clear them. Overlapping obstacle circles form their union.
Crowding correction accounts for the arena edge, but does not renormalize density
around obstacle boundaries. Plants can subsequently move under the shared
[physics rules](simulation.md); patch capacity remains fixed and growth uses
each plant's current position.

Patch-dependent growth requires a non-uniform world and varies individual plant
growth across its landscape. Growth is uniform by default, but plant count and
food production can still change. Compare [measured plant production](metrics.md)
as well as collected food. For a bounded run combining a world and movement choice:

```sh
cargo run --release --locked -- \
  --config configs/world/large-nonuniform.toml \
  --config configs/obstacles/scattered.toml \
  --config configs/brain/large.toml \
  --config configs/mutation/adaptive.toml \
  --config configs/locomotion/damped.toml \
  --config configs/movement/work-and-loss.toml \
  --seed 1 --ticks 1000
```

Energy units use a reference reproduction threshold of `1.0`, consistently for
animals, plants, food gains, and costs. Changing the configured threshold changes
the experiment, not its units. Earlier logs keep their original units.
The locomotion preset groups common `ecosystem.physics` settings with animal
motor limits and animal/plant drag. Energy-derived mass and contact behavior are
defined in the [simulation contract](simulation.md). The movement budget charges
positive translational and rotational work
separately, plus resistive motor losses. Braking earns no energy credit and
blocked thrust still incurs losses. Coasting with no actuation has no motor
charge; maintaining speed against drag requires paid actuation.
Work is divided by `motor_efficiency`; force and torque losses scale with their
squared magnitudes. `energy_scale` converts mechanical units to resource energy.
See the
[ecosystem settings](../src/ecosystem/config.rs) for the schema and
[simulation contract](simulation.md) for timing and state semantics.

Physical `time_step` advances shared dynamics and motor energy expenditure. Upkeep,
plant growth, replenishment, and reproduction remain per ecological tick;
`substeps` divides this interval into exactly that many fixed RK4 steps. Each
step evaluates forces four times. Choose a smaller step when increasing contact
stiffness or speed; integration does not automatically refine it. `tick_hz`
changes wall-clock pacing only.

Seed and model settings are immutable on [resume](persistence.md). Runtime
settings such as viewing, pacing, tracing, tick budget, checkpoint policy, and
metrics may change.
