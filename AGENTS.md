# Working on NEvo

- At the start of each session, read [README.md](README.md) for project context,
  decisions, and open questions. Read [experiments/README.md](experiments/README.md)
  when planning, running, or interpreting experiments.
- Keep instructions, notes, and documentation brief and concise. Maintain one
  authoritative home per fact; link instead of duplicating conversation history.
- Exercise independent judgment. When evidence supports a disagreement, state it,
  defend the reasoning, and explain the tradeoffs rather than agreeing reflexively.
  Persist where the concern remains valid; revise your position when evidence
  changes and respect the user's informed decisions.
- Update instructions, project knowledge, and relevant documentation as the code,
  understanding, and decisions evolve. Distinguish implemented behavior, agreed
  direction, proposals, and measured results; remove stale guidance.
- Preserve replaceability of agents and environments through small, explicit
  contracts. Keep experiment-specific body, sensing, action, and genome semantics
  out of universal interfaces. Discuss consequential design choices before coding.
- Treat NEvo as ongoing research with bounded, runnable milestones and experiments.
  Record experiment evidence under `experiments/`; do not infer success from an
  attractive visualization or turn a hypothesis into an established result.
- Keep simulation stepping independent of rendering. Check reproducibility and
  behavioral invariants when changing simulation rules; use checks proportional
  to the change. Documentation-only changes need content/link/whitespace checks.
- `wgame/` is a separate Git submodule with its own [instructions](wgame/AGENTS.md).
  Follow those when working there; keep NEvo-specific knowledge in this repository.
