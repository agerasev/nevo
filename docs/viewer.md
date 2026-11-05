# Drawing and viewing

The optional [viewer module](../src/viewer/mod.rs) uses wgame as its only drawing
backend. `Draw::draw(&self, &mut Drawing)` collects one default presentation for an
object. It must not change simulation state, consume randomness, or present frames.
`Simulation` has no drawing requirement; headless builds omit the viewer entirely.

`Drawing` holds a native wgame `Scene`, reusable `Library` access, background color,
optional world bounds, and text labels with optional colors. It retains drawing
instances and graphics handles, not references into simulation state. Bounds fit
into the target with preserved aspect ratio and a small margin; absent or invalid
bounds use the target's default camera. Labels contain presentation data only.

The [ecosystem implementations](../src/ecosystem/draw.rs) compose plants and bodies
into animals and the world. An `Animal<B, M>` draws its `B: Draw` body without any
bound on its mind. The ecosystem supplies the arena, world bounds, and population
labels, independently of its mind/genome types. [Universe](../src/universe/draw.rs)
adds the step label and delegates to its world.

Circle borders are slightly larger filled circles beneath every entity fill.
Overlapping entities therefore have no border crossing their interiors; animal
fills remain above plant fills. Individual bounds include the border.

The window has **World** and **Metrics** tabs. The [Metrics panel](metrics.md#viewing)
reads the run's CBOR log independently, including records appended after EOF.
Its searchable scalar plot supports full-history pan/zoom, hover values, and resume
boundaries. The viewer loop loads bounded batches into memory and downsamples
changed plots before UI layout, outside Universe locks. There is no separate
metrics worker.
The normal viewer skips world scene collection while Metrics is selected.
`viewer.tab` selects the initial tab; tabs do not change simulation execution.

The [window loop](../src/viewer/window.rs) knows no ecosystem types.
It collects the scene while holding a shared state read lock, releases the lock,
then lays out controls and renders. Neither `Clone` nor `Simulation` is required
to view a drawable state. Scene and model labels come from the same capture.
Pause/resume, single-step, and tick-rate changes use the command channel;
completion keeps the last state visible. Window options supply a title, static labels, and shutdown
signal. The application owns worker shutdown and joining.

`viewer.enable` attaches the window; `viewer.vsync` is passed to wgame's presentation
configuration and defaults to true. The window requests the next frame without an
additional timer. Simulation runs on its own worker, unlimited by default; optional
`tick_hz` pacing is independent of the viewer. Enabling viewing never changes it.

The toolbar's **Tick Hz** field changes pacing during the run; enter a positive
rate for paced playback or `0` for unlimited speed. **Full speed** toggles between
unlimited and the last positive rate (initially the configured rate, or 60 Hz).
Pause remains independent: editing speed while paused does not advance the world.
Speed commands wake an existing pacing wait and take effect between complete ticks.
Toolbar changes apply to the current invocation; saved `tick_hz` remains the startup
setting. No simulation timestep, randomness, or rendering rate is changed.

`show_with` accepts a custom capture callback for instrumentation. It runs once per
attempted frame, outside UI layout, and can finish after presenting its final scene.
The returned count includes only presented frames. The application's smoke checker
uses this path to issue commands before locking and verify complete captured state;
ordinary viewing needs no complete-state copy or verification bounds. The custom
capture still runs on every frame in either tab, allowing the same smoke checks
with `viewer.tab = "metrics"`.

Checks:

```sh
cargo test --locked --offline
cargo test --locked --offline --no-default-features
cargo test --locked --offline --test viewer_rendering -- --ignored --test-threads=1
cargo run --locked -- runs/viewer-check --config configs/debug.toml --config configs/smoke.toml
```

The explicit rendering tests require a GPU or software adapter and fail if none is
available. They check overlapping borders, pixels after source objects are dropped
or advanced, a second non-Clone model, and preservation of ecosystem state.
Viewer smoke checks require a display and verify twelve presented scenes with
pause/step/pacing controls.
