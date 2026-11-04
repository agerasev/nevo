# Working on NEvo

- At the start of each session, read [README.md](README.md) for project context,
  decisions, and open questions. Read the [history index](docs/history/README.md)
  when planning, running, or interpreting experiments.
- Keep instructions, notes, and documentation brief and concise. Maintain one
  authoritative home per fact; link instead of duplicating conversation history.
- Exercise independent judgment. When evidence supports a disagreement, state it,
  defend the reasoning, and explain the tradeoffs rather than agreeing reflexively.
  Persist where the concern remains valid; revise your position when evidence
  changes and respect the user's informed decisions.
- Treat tentative structural suggestions as discussion. Explain the recommendation
  and tradeoffs, and agree on consequential choices before implementing them.
- Update instructions, project knowledge, and relevant documentation as the code,
  understanding, and decisions evolve. Distinguish implemented behavior, agreed
  direction, proposals, and measured results; remove stale guidance.
- Preserve replaceability of agents and environments through small, explicit
  contracts. Keep experiment-specific body, sensing, action, and genome semantics
  out of universal interfaces. Discuss consequential design choices before coding.
- Prefer biological names for simulation components, such as `Animal.body` and
  `Animal.mind`.
- Group component contracts and implementations in their own Rust modules. If a
  module spans more than one file (including its root), put the root at
  `module/mod.rs`, never beside the directory as `module.rs`.
- Documentation describes the current state, including clearly labeled current
  plans and open questions. Keep only valuable, concise historical conclusions
  in `docs/history/`; use Git history for earlier implementations and routine
  infrastructure increments.
- Treat NEvo as ongoing research with bounded, runnable milestones and experiments.
  Historical experiment notes contain the main idea and main results. Do not infer
  success from a visualization or turn a hypothesis into an established result.
- Store run artifacts, including screenshots, alongside checkpoints in the ignored
  `runs/` directory. Keep large generated files out of Git; do not add image-wide
  ignore rules. Commit small reusable configs under `configs/` and useful scripts.
- Keep simulation stepping independent of rendering. Check reproducibility and
  behavioral invariants when changing simulation rules; use checks proportional
  to the change. Documentation-only changes need content/link/whitespace checks.
- Commit each completed editing milestone after its relevant checks pass.
- `wgame/` is a separate Git submodule with its own [instructions](wgame/AGENTS.md).
  Follow those when working there; keep NEvo-specific knowledge in this repository.
