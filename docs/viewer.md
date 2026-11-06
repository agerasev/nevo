# Drawing and viewing

The optional viewer uses wgame as its sole drawing backend. Objects supply one
default presentation through [Draw](../src/viewer/mod.rs); ecosystem presentation
stays separate from generic window controls. See the Rust contract for purity and
ownership guarantees. Drawing must remain independent of simulation stepping.

Use **Pause / Resume**, **Single step**, and **Tick Hz** to control execution.
With the arena focused, Space toggles playback and Right Arrow steps. **Full speed**
toggles unlimited execution and the previous paced rate. Changing speed while
paused does not advance the world. Toolbar changes apply only to this invocation;
the saved pacing setting remains the startup value.

**World** shows plants in green and animals in yellow, with a marker indicating
each animal's heading. **Metrics** shows recorded
[history and plots](metrics.md#viewing). Completion leaves the final state visible;
closing the window stops the worker. `viewer.vsync` controls presentation without
changing simulation time or pacing.

In **World**, drag with the left mouse button to pan and scroll to zoom around the
cursor. **Fit world** or Home with the arena focused restores automatic fitting.
Manual navigation preserves the center and magnification when resizing or switching
tabs. Camera state belongs to the viewer and is not saved in simulation checkpoints.
Try a [large non-uniform world](../configs/world/large-nonuniform.toml) with
`--viewer --paused`.

The [rendering tests](../tests/viewer_rendering.rs) require a GPU or software adapter:

```sh
cargo test --locked --test viewer_rendering -- --ignored --test-threads=1
```
