# Headless foraging trial and continuous-run comparison

**Completed November 3, 2026.** Does stopping at a trial budget change any of
the first N ticks, or the following tick when the same trial is resumed?

Built the clean tree at `06c0c4721247b6dece99669cb78435358fe35494` with the
committed lockfile and `rustc 1.98.1` on `x86_64-unknown-linux-gnu`.
The pinned wgame revision was `58e322799a2d7448e85a8762c97801b60a8ced78`.
Both runs used master seed 1 and started independently with identical scenarios.

```sh
cargo build --locked --no-default-features --bin forage
target/debug/forage trial --scenario debug --seed 1 --ticks 1
target/debug/forage check --scenario debug --seed 1 --ticks 128
target/debug/forage check --scenario baseline --seed 1 --ticks 256
target/debug/forage trial --scenario baseline --seed 1 --ticks 256
target/debug/forage continuous --scenario baseline --seed 1 --ticks 256
```

The debug founder ate a ripe plant for 40 energy on tick 1. The parent and its
mutated child both acted with 213 energy. Immigration was disabled. The two
animals died by tick 109 and the empty world continued to the budget.

| Scenario | Ticks compared | Result |
| --- | ---: | --- |
| One founder in contact with one plant | 128 | All reports and complete states matched, including resumed tick 129. |
| 20 founders, 500 ripe plants, immigration floor 20 | 256 | All reports and complete states matched, including resumed tick 257. |

At tick 256 the standalone baseline commands both reported 193 meals, 3 births,
38 deaths, and 35 immigrants, with 20 living animals and 500 plants. The checks
cover events, actions, bodies, plant energy, neural memory, genomes, and random
generator state. They establish runner equivalence for these runs on this build.
