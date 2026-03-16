# Beginner Rebuild Plan for `handarbeit/`

## Summary
Rebuild the `ai_generated/` example in `handarbeit/` as a sequence of 6 larger milestones, each ending in a running example. The teaching order should favor visible progress and low conceptual load first, then introduce the harder architectural ideas once the learner already has a window, drawing, input, and text working.

The final state should match the current `ai_generated/` crate in spirit:
- window + event loop
- rectangle renderer
- real text measurement/rendering with FreeType + HarfBuzz
- immediate-mode UI rebuild each frame
- stable widget IDs and retained per-widget cache
- child-driven layout with separate sizing and placement passes

Defaults chosen for this plan:
- 5-6 larger steps
- real text introduced midway

## Milestones

### Step 1 — Window, event loop, and clear color [completed]
Goal: replace `Hello, world!` with a real native window and a render loop.

Implement:
- Add `winit`, `wgpu`, and `pollster`.
- Create a small `App`/`GpuState` shape similar to the reference, but only enough to:
  - open a window
  - initialize `wgpu`
  - handle resize
  - redraw continuously
  - clear to a solid background color
- Keep all code in a very small set of files at first: `main.rs` plus one GPU/app helper module if needed.

Running result:
- A window opens and continuously redraws with a stable background color.

What the learner should understand after this step:
- what `winit` does
- why `wgpu` setup is async
- why `pollster::block_on` exists
- the basic event loop / redraw flow

### Step 2 — First rectangle and minimal 2D geometry [completed]
Goal: draw visible geometry before introducing any UI abstraction.

Implement:
- Add a tiny geometry layer:
  - `Vec2`
  - `Rect`
  - `Color`
  - screen-to-NDC conversion
- Add a minimal rectangle draw path in the GPU module:
  - `DrawCmd::Rect`
  - a vertex type
  - CPU tessellation of rectangles into triangles
- Hardcode one or two rectangles from `main` or a tiny demo function.

Running result:
- A window with one or two colored rectangles drawn in fixed positions.

What the learner should understand:
- the difference between app logic and draw commands
- CPU-side tessellation into GPU vertices
- why a simple immediate draw list is useful

### Step 3 — Real text, introduced with FreeType + HarfBuzz [completed]
Goal: get rid of fake text early enough that later layout uses real font metrics.

Implement:
- Add `freetype-rs` and `harfbuzz_rs_now`.
- Add a `text` module that:
  - loads one known font file from the system
  - shapes text with HarfBuzz
  - rasterizes glyph bitmaps with FreeType
  - measures text width/height from shaped glyphs
- Keep rendering simple:
  - convert glyph bitmap pixels into many tiny rects
  - continue using the existing rectangle renderer
- Add `DrawCmd::Text`.
- Replace any hardcoded bitmap-font logic with real shaping/rasterization.

Running result:
- The window shows a rectangle plus real font-rendered text.

What the learner should understand:
- FreeType vs HarfBuzz responsibilities
- why text measurement must come from the same system that renders text
- that this is intentionally a simple but inefficient text renderer

### Step 4 — Mouse input and a first interactive button [completed]
Goal: make the app feel alive before introducing the full retained/immediate architecture.

Implement:
- Add per-frame mouse input state:
  - cursor position
  - mouse down
  - pressed this frame
  - released this frame
- Build one manual button with:
  - a fixed rectangle
  - hover/pressed color changes
  - click detection
  - a counter or status text that changes when clicked
- Do not introduce IDs or retained widget cache yet.
- Keep the button demo straightforward and explicit.

Running result:
- A clickable button changes appearance on hover/press and updates a visible count or label.

What the learner should understand:
- frame-based input handling
- the difference between hover, active press, and click
- how immediate interaction can work even before a UI system exists

### Step 5 — Immediate-mode UI scaffolding and retained widget identity [completed]
Goal: transition from manual demo code to the article’s core idea: rebuild every frame, keep widget state separately.

Implement:
- Introduce a small `ui` module with:
  - `Ui`
  - `UiMemory`
  - stable widget IDs derived from string sources
- Convert the manual button into `ui.button("button", ...)`.
- Add a retained per-widget cache keyed by widget ID.
- Store at least:
  - last touched frame
  - last known rect
- Use the retained rect for interaction on subsequent frames.
- Keep layout simple for this step:
  - manual positions or one root panel with fixed origin
  - no child-driven sizing yet

Running result:
- The same button now comes from a small immediate-mode UI API rebuilt each frame.
- Widget identity persists across frames through the retained cache.

What the learner should understand:
- stable widget identity
- why the UI tree is rebuilt every frame
- why retained state lives outside the transient build code

### Step 6 — Tree build, child-sized layout, and final article-shaped demo [completed]
Goal: implement the part that makes the article’s layout story real.

Implement:
- Change `Ui` from emitting commands directly to building a small node tree first.
- Add node kinds for at least:
  - root panel
  - label
  - button
  - free rect / free text if still needed for the frame counter and background
- Run layout in two passes:
  - bottom-up size computation
  - top-down placement
- For panels, compute width from children:
  - width = widest child + horizontal padding
  - height = summed child heights + spacing + vertical padding
- Keep the demo intentionally small:
  - frame counter outside the panel
  - one root panel
  - one label
  - one button whose text width changes as the count grows
- End at the same conceptual state as `ai_generated/`.

Running result:
- The panel width visibly depends on its children.
- The button label changes with the counter.
- The UI is rebuilt every frame while retained widget state survives across frames.

What the learner should understand:
- why child-driven sizing requires a separate pass
- why a tree is useful even in an immediate-mode system
- how the retained cache and the per-frame tree solve different problems

### Step 7 — Replace custom math with `glam` and evaluate renderer upgrades
Goal: swap the tiny hand-written math layer for a real vector math crate after the core UI architecture is already understood, then briefly evaluate what a more production-like rendering path could look like.

Implement:
- Add `glam`.
- Replace the custom `Vec2` type with `glam::Vec2`.
- Update geometry, GPU, text, and UI code to use `glam` vector operations.
- Keep `Rect` as a small local type unless using a crate-backed rectangle abstraction is clearly better.
- Do not change the overall UI architecture in this step; this is a math-layer refactor only.
- As a follow-up exploration, inspect what it would mean to replace parts of the hand-written renderer with:
  - `lyon` for path tessellation
  - `vello` for higher-level GPU vector rendering
  - Skia as a full rendering backend
- Keep this exploration separate from the main teaching implementation unless there is a clear decision to migrate.

Running result:
- The final demo still behaves the same, but uses `glam` for vector math instead of the homemade `Vec2`.
- There is a short written comparison of what would change if the renderer later moved toward `lyon`, `vello`, or Skia.

What the learner should understand:
- where a small custom math type is enough for learning
- when it becomes more practical to adopt a real math crate
- how to separate a math refactor from architectural changes
- the difference between a math crate (`glam`), a tessellation library (`lyon`), a vector renderer (`vello`), and a full graphics backend (Skia)

### Step 8 — Reuse GPU vertex buffers instead of reallocating every frame
Goal: keep the immediate-mode rebuild model, but make the upload path more realistic by reusing GPU buffers across frames.

Implement:
- Keep tessellating the draw list every frame on the CPU.
- Store a persistent vertex buffer in `GpuState`.
- Track the current vertex capacity separately from the number of vertices used this frame.
- Replace per-frame `create_buffer_init` allocation with:
  - `queue.write_buffer(...)` when the existing buffer is large enough
  - buffer reallocation only when the vertex count exceeds capacity
- Draw using only the active vertex count for the current frame.
- Keep the architecture otherwise unchanged; this step is a render-path optimization, not a UI redesign.

Running result:
- The demo still renders exactly as before.
- The renderer no longer creates a brand new vertex buffer on every frame unless it needs to grow.

What the learner should understand:
- why immediate-mode UIs still commonly regenerate geometry every frame
- why GPU buffer reuse is separate from CPU-side tessellation
- how capacity-based buffer growth avoids per-frame allocation overhead
- where this optimization fits in the renderer without changing the higher-level architecture

### Step 9 — Handle shutdown signals and exit cleanly
Goal: make the app terminate predictably when the process is asked to stop, instead of relying only on window-close behavior.

Implement:
- Add explicit shutdown handling for normal window close and external termination requests.
- Track shutdown intent in the app state so the event loop can stop requesting redraws once exit begins.
- Ensure render/update code stops touching GPU/window state after shutdown has started.
- Release long-lived app resources in a controlled order during teardown.
- Test behavior with at least:
  - normal window close
  - `SIGINT`
  - `SIGTERM`
- Document any platform limitations around signals, event-loop wakeups, or forced termination.

Running result:
- Closing the window exits cleanly.
- Sending a normal termination signal causes the app to stop its loop and exit without crashing.

What the learner should understand:
- the difference between cooperative shutdown and forced termination
- why GUI apps need to stop scheduling new work before tearing resources down
- what signals can realistically be handled safely in a Rust/winit app
- why `SIGKILL` cannot be handled and must be treated differently from catchable signals

### Step 10 — Move text rendering from pixel-rect tessellation to a glyph atlas
Goal: keep HarfBuzz shaping and FreeType rasterization, but replace the teaching-only per-pixel rectangle path with a more realistic text renderer.

Implement:
- Keep `text::measure(...)` and shaping logic based on HarfBuzz + FreeType metrics.
- Rasterize glyph bitmaps with FreeType and cache them in a GPU texture atlas.
- Store per-glyph atlas metadata:
  - atlas UV rect
  - bitmap size
  - bearing / offset
  - advance
- Change text rendering from “one rect per bitmap pixel” to “one textured quad per glyph”.
- Extend the render pipeline to sample glyph alpha from the atlas in the fragment shader.
- Batch text quads efficiently while preserving the existing rectangle path for non-text primitives.
- Decide how atlas growth/eviction should work for this small project and document the tradeoff.

Running result:
- The app renders the same text as before, but with drastically less geometry.
- Text rendering cost scales roughly with glyph count, not bitmap pixel count.

What the learner should understand:
- why the current per-pixel-rect path is useful for learning but not realistic for performance
- how a glyph atlas separates rasterization cost from draw cost
- why one quad per glyph is the common simple text-rendering path
- what new renderer complexity is introduced by textures, UVs, and glyph caching

## Implementation Notes
Use the same `handarbeit/` crate throughout. Each step should build on the previous one rather than creating separate crates.

Recommended module growth over time:
- Step 1: `main.rs`, maybe `app.rs`/`gpu.rs`
- Step 2: add `geom.rs`
- Step 3: add `text.rs`
- Step 5: add `ui.rs`
- Step 6: keep the same modules, but change `ui.rs` from direct emission to tree + layout passes

Dependency introduction order:
- Step 1: `winit`, `wgpu`, `pollster`
- Step 2: `bytemuck`
- Step 3: `freetype-rs`, `harfbuzz_rs_now`
- Step 7: `glam`

Keep every step runnable with `cargo run` from `handarbeit/`.

## Test / Acceptance Criteria
For each milestone:
- `cargo check` passes
- `cargo run` opens a working window in a graphical environment
- the new visible behavior for that step is clearly demonstrable

Per-step acceptance:
- Step 1: window opens and redraws
- Step 2: at least one rectangle is visible
- Step 3: text renders from a real font and is measurably wider than the old bitmap approach
- Step 4: button hover/press/click works
- Step 5: widget interaction still works after converting to stable IDs + retained cache
- Step 6: panel width changes based on child content
- Step 7: final demo still works after replacing the custom vector type with `glam::Vec2`

## Assumptions
- `handarbeit/` is the learning crate and will be built up incrementally from empty.
- `ai_generated/` remains the reference implementation to compare against, not the teaching target.
- The learner is a very early beginner, so each milestone should prioritize one new idea at a time over abstraction purity.
- Text rendering may remain inefficient in the final hand-coded version if it keeps the architecture understandable.
