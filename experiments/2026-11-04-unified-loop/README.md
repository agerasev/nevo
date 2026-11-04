# One loop with an optional viewer

**Completed November 4, 2026.** Replace separate execution paths with one
parameterized loop and a read-only viewer. The user also chose sensing before
feeding and births, deliberately changing the prior baseline's tick order.

Source: the commit containing this record, tested as working changes after
`10ac3e2`. The lockfile and wgame pin `58e322799a2d7448e85a8762c97801b60a8ced78`
are unchanged. Rust/Cargo 1.98.1 on `x86_64-unknown-linux-gnu`.

```sh
cargo test --locked --offline
cargo test --locked --offline --no-default-features
cargo clippy --locked --offline --all-targets -- -D warnings
cargo run --locked --bin nevo -- --scenario debug --seed 1 --ticks 1 --hz 0
cargo run --locked --bin nevo -- --scenario baseline --seed 1 --ticks 256 --hz 0
cargo run --locked --bin nevo -- --scenario baseline --seed 42 --ticks 256 --hz 0
cargo run --locked --bin nevo -- --scenario debug --seed 1 --focal 1 --hz 0
```

All 19 tests pass with and without graphics. Lock checks verify that agents and
callbacks hold no world lock, readers can inspect a slow agent's input world, and
a held read guard delays publication. Controls acknowledge completed steps;
pausing, resuming, zero budgets, and paused wall-time limits work. Limited and
condition-stopped executions match every report and final state through 128 ticks
for both scenarios at seed 42, including continuation. These are comparisons
within the new ordering, not equivalence with the old baseline.

Fresh runs use unchanged initialization, mutation, immigration, and energy
parameters. No external selection or resets occur within a run. At debug seed 1,
tick 1 has one decision with food strength 100 and energy 390. Feeding and birth
leave two bodies at 213 energy, with the child still at zero recurrent memory.
The focal founder dies at tick 109; the empty world remains available.

| Baseline seed | Tick budget | Meals | Births | Deaths | Immigrants | Living |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 256 | 184 | 4 | 42 | 38 | 20 |
| 42 | 256 | 196 | 0 | 34 | 34 | 20 |

The viewer was checked on isolated Xvfb with Mesa's software Vulkan adapter:

```sh
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
  WGPU_BACKEND=vulkan LIBGL_ALWAYS_SOFTWARE=1 \
  xvfb-run -a -s '-screen 0 1280x1024x24' \
  timeout 40s target/debug/nevo --viewer --scenario debug --seed 1 --smoke
# Repeat with --scenario baseline --seed 42.
```

Both runs passed twelve presented-snapshot comparisons at their sampled ticks,
including pause, step, and resume commands. Rendering timing can change which
ticks are sampled, but not their state. Circle strokes now expand in local
coordinates before translation, preserving the fill center and overlap borders.
Inspected captures: [baseline at tick 0](viewer-baseline.png) and
[debug offspring at tick 1](viewer-debug.png).
These short checks establish execution/observation consistency, not adaptation.
