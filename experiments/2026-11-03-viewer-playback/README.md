# Viewer and headless playback

**Completed November 3, 2026.** Does presenting, pausing, or stepping the viewer
change the foraging state reached at a given tick?

Implementation `bd26e95b264a2baeb5762af6df9fcfdf21006028`, wgame
`58e322799a2d7448e85a8762c97801b60a8ced78`, locked dependencies, Rust 1.98.1 on
x86_64 Linux. Window checks used a private Xvfb display and Mesa software Vulkan.

```sh
cargo build --locked --bin nevo
cargo test --locked --bin nevo
WGPU_BACKEND=vulkan xvfb-run -a target/debug/nevo --scenario debug --seed 1 --paused --smoke
WGPU_BACKEND=vulkan xvfb-run -a target/debug/nevo --scenario baseline --seed 42 --smoke
```

Both window runs presented ticks `0, 0, 3, 3, 3, 4, 5, 5, 6, 8, 9, 9` during a
scripted sequence of play, pause, step, and resume. After every presentation the
complete viewer run equaled the independently initialized headless run at that
tick, including controller memories and RNG states. A separate playback test
compared seed 42 through tick 128, including immigration.

With real mouse and keyboard input on the debug scenario, the paused window held
at tick 0. Clicking **Single step** showed tick 1 and two herbivores. Resume
advanced the tick, Pause held it steady, Right Arrow stepped once with the arena
focused, and Space resumed and paused playback.
