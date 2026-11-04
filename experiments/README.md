# Experiments

Keep the research history here. Use one `YYYY-MM-DD-short-name/` directory per
experiment. It may contain several runs, seeds, and comparisons.

- [The first NEvo foraging experiment](2016-07-26-herbivore-foraging/README.md) reconstructs
  the 2016 C++ experiment from source and links to the author's long-run recordings.
- [Headless trial and continuous-run comparison](2026-11-03-foraging-prefix/README.md)
  checks the first Rust birth trace and seeded replay through population turnover.
- [Viewer and headless playback](2026-11-03-viewer-playback/README.md) checks
  seeded state after presenting frames, pausing, resuming, and single-stepping.
- [Draft trait reuse](2026-11-04-draft-traits/README.md) compares seeded trajectories
  before and after restoring the agent and genome contracts.
- [One loop with an optional viewer](2026-11-04-unified-loop/README.md) checks
  read–act–write locking, shared execution controls, and the new sensing-first order.

Each experiment needs a short `README.md` recording:

- Question/hypothesis and the changed factor or baseline.
- Reproducibility: source and submodule revisions, any uncommitted source
  changes, configuration, seeds, execution command, and relevant platform details.
- Protocol: world reset policy, selection/reproduction policy, step/evaluation
  budget, stopping conditions, and measured quantities.
- Results and artifact paths, including failures/extinction; a brief interpretation
  and next question. Label planned, running, and completed work accurately.

Store small configs, metrics, and selected genomes beside the note. Reference large
artifacts instead of committing them by default. Preserve earlier results; correct
mistakes explicitly. Long runs should log periodic metrics and resumable checkpoints
once supported, including world, controller memory, and RNG state.

For perception ablations, use matched environment seeds, evaluation budgets, and
evolution settings. Record input masking, network capacity, and whether each variant
was evolved independently or only altered during evaluation. Distinguish environment
termination from the runner's time limit; record any automatic reseeding or restart.
