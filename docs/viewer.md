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

**World** shows plants in green and animals in yellow. **Metrics** shows recorded
[history and plots](metrics.md#viewing). Completion leaves the final state visible;
closing the window stops the worker. `viewer.vsync` controls presentation without
changing simulation time or pacing.

The [rendering tests](../tests/viewer_rendering.rs) require a GPU or software adapter:

```sh
cargo test --locked --test viewer_rendering -- --ignored --test-threads=1
```
