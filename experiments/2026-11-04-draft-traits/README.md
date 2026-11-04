# Restoring the draft traits

**Completed November 4, 2026.** Does restoring `Agent`, `Genome`, `Mutate`, and
their `Context` contract preserve the baseline's seeded trajectories?

Compared `f58583989a14a14c50eb9aa4c5a4a9a0741fdec2` with the implementation in
the commit containing this note, measured as its working tree before commit.
Both use their locked dependencies and unchanged wgame
`58e322799a2d7448e85a8762c97801b60a8ced78`. Toolchain: rustc 1.98.1
(`48a229cea`), Cargo 1.98.1, `x86_64-unknown-linux-gnu`.

```sh
python3 experiments/2026-11-04-draft-traits/compare.py
# To compare committed revisions, add --before REV --after REV.
```

The [script](compare.py) builds both snapshots offline with graphics disabled.
It uses the checkout's wgame submodule for Cargo path resolution. Temporary
sources, builds, and traces are removed afterward. The [trace program](trace.rs)
starts fresh debug and baseline runs with seeds 1 and 42, records tick zero and
128 ticks each, and uses the unchanged reproduction and immigration rules.
There is no early stopping or external selection.

All reports, complete world states, live genomes, recurrent memories, and both
RNG states matched at every recorded tick. The genetic RNG is extracted from its
debug representation so its new context wrapper does not affect comparison.
Both traces were 18,714,694 bytes with SHA256
`bcda48ffab0f1b1fa238aafd910c4d9b7f7cbb82b4b64c110e272acf114485bf`.

| Scenario | Seed | Meals | Births | Deaths | Immigrants |
| --- | ---: | ---: | ---: | ---: | ---: |
| Debug | 1 | 1 | 1 | 2 | 0 |
| Debug | 42 | 1 | 1 | 2 | 0 |
| Baseline | 1 | 119 | 2 | 11 | 10 |
| Baseline | 42 | 112 | 0 | 17 | 17 |

The refactor preserves these trajectories through mutation, immigration, and
debug extinction on this build. This does not establish portability across
platforms or behavior beyond the measured runs. Future interchangeable agent
implementations can use these runs as a baseline comparison.
