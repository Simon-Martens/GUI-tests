# Rebuild Plan for `ai_generated`

## Overview
- Goal: small retained-root / immediate-tree UI with `taffy`, hover, click, clipping, and retained counters.
- Pipeline: `render -> request_layout -> prepaint -> resolve interaction -> paint -> GPU`.
- Paint boundary: UI emits only `Rect` and `Text` draw commands.
- State split: retained state lives in `UiMemory`; frame-local state lives in `Window`.

## Constraints
- Keep: `winit`, `wgpu`, FreeType, HarfBuzz, `taffy`, `euclid`.
- Support only: quads, text, buttons, vertical containers, hover, click.
- Exclude: images, keyboard/text input, selection, scrolling, focus, drag and drop.

## File Layout
- `src/main.rs`
- `src/app.rs`
- `src/geom.rs`
- `src/gpu.rs`
- `src/text.rs`
- `src/ui.rs`

## Dependencies
- `bytemuck` with `derive`
- `freetype-rs`
- `euclid`
- `harfbuzz_rs_now`
- `pollster`
- `taffy`
- `wgpu` with `wgsl`
- `winit`

## Rebuild Sequence

### Stage 0. Empty Crate, Builds Cleanly
- Status: complete
- Summary: crate, modules, dependencies, and `cargo check` baseline.

### Stage 1. Window + GPU + One Rectangle
- Status: complete
- Summary: `geom.rs`, `gpu.rs`, `app.rs`, redraw loop, and basic rect rendering.

### Stage 2. Add Text, Still Without UI Abstractions
- Status: complete
- Summary: text measurement/rasterization plus `DrawCmd::Text`.

### Stage 3. Add Retained Frame State, Still Without Elements
- Status: complete
- Summary: `UiMemory`, widget bookkeeping, and retained counters.

### Stage 4. Add a Root `Render` View
- Status: complete
- Summary: retained root view object owned by the app shell.

### Stage 5. Add the Frame `Window` Object
- Status: complete
- Summary: frame-owned UI context with screen helpers and counter accessors.

### Stage 6. Add a Minimal Element Trait With One Leaf Element
- Status: complete
- Summary: `Element` trait plus `Quad` as the first leaf.

### Stage 7. Add `AnyElement`
- Status: complete
- Summary: type-erased element wrapper with typed per-element phase state.

### Stage 8. Add Root Phase Driving
- Status: complete
- Summary: root uses the same prepaint/paint path as child elements.

### Stage 9. Add `taffy`
- Status: complete
- Summary: root and leaf layout moved onto `taffy`.

### Stage 10. Add `IntoElement` and `ParentElement`
- Status: complete
- Summary: delayed type erasure and fluent child composition.

### Stage 11. Add `Div` as the First Real Container
- Status: complete
- Summary: vertical container with position, size, padding, gap, background, clip, and children.

### Stage 12. Add Absolute Text
- Status: complete
- Summary: absolute positioned text as a second leaf type.

### Stage 13. Add Flow `Label`
- Status: complete
- Summary: measured text that participates in container layout.

### Stage 14. Add Button Layout and Paint, But No Input Yet
- Status: complete
- Summary: intrinsic button sizing and visual paint without interaction.

### Stage 15. Add Stable IDs
- Status: complete
- Summary: local/global widget IDs and stable scoped hashing across frames.

### Stage 16. Add Hitboxes
- Status: complete
- Summary: clickable/blocking hitboxes collected during prepaint.

### Stage 17. Add Content Masks
- Status: complete
- Summary: clipping masks applied to paint and hit testing.

### Stage 18. Add Frame Interaction Resolution
- Status: complete
- Summary: frame-local `hovered`, `active`, and `clicked` resolved after prepaint.

### Stage 19. Add Button Interaction
- Status: complete
- Summary: button hover/press visuals and click-triggered action emission.

### Stage 20. Add Actions and `UiOutput`
- Status: complete
- Summary: UI emits `draw_list`, `actions`, and `interaction`; app mutates state later.

### Stage 21. Add `Window::draw`
- Status: complete
- Summary: full frame pipeline centralized inside `Window`.

### Stage 22. Wire the App Shell to the New UI Pipeline
- Status: complete
- Summary: app shell calls `window.draw(...)`, applies actions, then renders output.

### Stage 23. Build the Minimal Demo
- Status: complete
- Summary: retained demo view with background, absolute primitives, nested panel layout, and clickable counters.

## Runtime Validation Checklist
- Window opens and redraws.
- Background renders.
- Absolute rectangle renders.
- Text renders.
- Panels size from children under `taffy`.
- Button hover changes color.
- Press-inside / release-inside triggers once.
- Press-inside / release-outside does not trigger.
- Blocking content prevents input behind it.
- Clipped content does not paint or hit-test outside its mask.
- Counters persist across frames.

## Explicitly Not Allowed
- No double-build frame hack.
- No `button() -> bool` second-pass API.
- No global parallel layout/prepaint side arrays.
- No layout decisions in paint.
- No previous-frame hit testing.
- No root-only phase bypass.
- No app-state mutation inside paint.

## Minimum Viable Completion Criteria
- Retained root implements `Render`.
- Root returns an `AnyElement` tree.
- Root and children share the same `Element` contract.
- `taffy` is the only layout engine.
- Interaction is resolved from current-frame hitboxes after prepaint.
- Buttons register click metadata during prepaint.
- Paint emits only rect/text draw commands.
- Renderer consumes only draw commands.
- App applies emitted actions after frame production.

## Next Steps After MVP
These are intentionally out of scope for the initial rebuild, but they are the next rendering upgrades to make after the MVP is stable.

### Next Step 1. Replace Pixel-Rect Text With Atlas Sprites
Goal:
- stop expanding every glyph pixel into a tiny rectangle
- move toward the GPUI-style text path

Implement:
- glyph atlas texture management
- glyph cache entries keyed by font/text render parameters
- sprite-based text draw commands instead of per-pixel rect expansion

Why this is next:
- the current text path is structurally acceptable for MVP
- it is the biggest obvious performance weakness
- atlas sprites improve both performance and renderer shape without changing the UI phase model

Expected architectural changes:
- add sprite-oriented paint commands
- make text painting emit glyph sprite draws
- keep shaping and measurement in the text system
- keep the UI phases unchanged

### Next Step 2. Introduce a Richer Scene / Batch Model
Goal:
- stop treating the final paint output as just one flat list of immediate draw commands
- keep higher-level render primitives around longer

Implement:
- a `Scene` or equivalent render list object
- typed primitive buckets or batches
- batching by primitive kind and shared render state

At minimum, the richer scene should separate:
- solid quads
- text sprites
- later clip/layer primitives if added

Why this is next:
- once text uses atlas sprites, a richer scene becomes much more useful
- batching becomes easier and more explicit
- this is the point where the renderer starts to resemble GPUI more closely

Expected architectural changes:
- `paint` emits scene primitives instead of directly pushing final GPU-facing commands
- renderer consumes the finalized scene/batches
- clipping should move toward a scene-level concept instead of staying baked into individual commands

### Next Step 3. Introduce an App / Entity / Context Ownership Model
Goal:
- stop treating the app as one retained root object plus ad hoc UI memory
- separate long-lived application state from frame-local window state and element-local transient state

Implement:
- `App` as the owner of long-lived models and views
- typed `Entity<T>` handles for retained objects
- `Context<T>` for entity-scoped services and mutation
- notification / subscription / typed event hooks between entities
- window roots stored as handles to retained views rather than raw view structs

Why this is next:
- once the MVP has more than one meaningful retained object, shared state and cross-component communication become the next architectural pressure point
- this is the main missing layer between the MVP plan and GPUI's broader software architecture

Expected architectural changes:
- retained state moves out of ad hoc root fields or `UiMemory` maps and into app-owned entities
- `Render` is implemented by entity-backed views
- frame-local `Window` remains frame-local
- `Element`, `request_layout`, `prepaint`, and `paint` stay unchanged
- actions target entity updates through contexts instead of directly mutating root-local state

### Next Step 4. Add Retained Render Subtree Caching
Goal:
- reuse previously produced layout/prepaint/paint output for stable retained subtrees
- move beyond full-frame regeneration without introducing screen-tile caching

Implement:
- a retained render boundary type backed by an entity/view handle
- cache keys based on subtree inputs such as bounds, content mask, text style, and dependency dirtiness
- stored reusable ranges for:
  - prepaint output
  - paint/scene output
- dirty tracking so cached subtrees are invalidated when observed state changes

Examples of good retained subtree boundaries:
- a sidebar
- a status bar
- a menu or popup root
- an editor pane
- a large panel or window section

Why this is next:
- this is much closer to GPUI than caching arbitrary screen regions
- views in this model are broad retained UI units, not tiny render primitives
- subtree reuse preserves the existing layout/prepaint/paint model
- it improves performance without introducing tile invalidation, compositing complexity, or stale hit-testing bugs

Expected architectural changes:
- root still rebuilds the logical tree each frame
- some retained subtree nodes can skip fresh render/prepaint/paint work when clean
- scene output can be replayed from the previous frame for cached subtrees
- hitboxes, cursor state, and input handlers remain tied to reused prepaint/paint output

### Next Step 5. Add Dirty / Invalidation-Driven Redraw
Goal:
- stop rebuilding frames continuously when nothing changed
- redraw only when input, window state, or retained app state invalidates the frame

Implement:
- a coarse `needs_redraw` or `dirty` flag in the app shell
- request redraw on:
  - startup
  - resize / scale changes
  - cursor movement or cursor leaving the window
  - mouse press / release
  - app-state mutations caused by emitted `UiAction`s
- stop using an unconditional poll loop for redraw

Rules:
- idle windows do not redraw
- input events that can affect visuals invalidate the next frame
- applying actions after `UiOutput` may request a follow-up redraw if retained state changed
- this is coarse invalidation only, not yet entity/view dependency tracking

Why this is next:
- it is the first real performance step borrowed from GPUI
- it reduces work immediately without changing the UI phase model
- finer-grained invalidation can come later with retained entities/views

### Next Step 6. Add Optional Debug Frame Timing
Goal:
- measure how long a frame takes without baking frame counters into the UI itself
- make frame timing opt-in through a debug setting

Implement:
- a small app-level debug options struct
- a `time_frames` flag
- CPU-side frame timing with `std::time::Instant`
- console logging in the redraw path that prints:
  - frame number
  - total frame time
  - a coarse app-shell breakdown such as:
    - draw
    - actions
    - render
    - finish
- then split the expensive buckets further when needed:
  - UI draw:
    - render tree construction
    - prepaint
    - interaction resolution
    - paint
  - GPU render:
    - surface acquire
    - tessellation
    - buffer upload
    - command encoding
    - submit / present

Rules:
- timing is disabled by default unless the debug option is set
- this measures end-to-end app-side frame production and submission time, not true GPU execution time
- no frame counter should be exposed through `Window` just for diagnostics

Why this is next:
- it gives immediate visibility into frame cost while keeping debugging concerns out of the UI model
- it is cheap to add and useful while developing the later performance steps

### Next Step 7. Add Stutter-Resistant Latest-State Frame Scheduling
Goal:
- avoid trying to "catch up" by effectively replaying missed frames after a short stall
- make the UI recover by rendering the newest state once CPU time becomes available again

Implement:
- a latest-state-wins scheduling policy in the app/window frame driver
- at most:
  - one frame in progress
  - one pending dirty bit for "draw again after this frame"
- if invalidation happens while drawing, do not queue another full render job
- instead mark the frame as dirty again and render once more from current state after the current frame completes
- make animation and time-based UI state derive from wall-clock time, not from counting rendered frames

Rules:
- no queue of pending frame jobs
- no attempt to display every missed intermediate state
- after a short stall, render the newest state directly
- animation progression must be based on elapsed time so missed frames collapse naturally

Why this is next:
- this makes the UI more resistant to short CPU stalls and transient contention
- for GUI workloads, jumping to the latest state is usually less noticeable than replaying intermediate updates
- it complements dirty/invalidation-driven redraw instead of replacing it

### Next Step 8. Add Vector Path Primitives and Tessellation
Goal:
- support non-rectangular vector primitives without hand-writing custom tessellation code
- move beyond quads and text sprites when the renderer needs true paths

Implement:
- a path primitive in the scene or paint output
- fill and stroke support for paths
- path tessellation at the render preparation boundary
- likely use `lyon` for path construction and tessellation instead of maintaining a custom tessellator

Examples of where this helps:
- custom icons and symbols
- curved separators or callouts
- richer borders and outlines
- underlines or decorations that stop being simple rects
- arbitrary bezier or polygon content

Rules:
- this is for vector/path rendering, not for text glyph rendering
- text should still move toward atlas sprites rather than path tessellation
- path tessellation output should feed the richer scene/batch model instead of bypassing it

Why this is next:
- `lyon` is a pragmatic way to add path support without taking on a large geometry algorithm project
- it becomes much more useful once a richer scene exists
- it is optional unless the UI starts needing real vector geometry

### Next Step 9. Evaluate a Skia-Backed Renderer
Goal:
- allow the renderer backend to target a mature 2D GPU engine instead of being tied to `wgpu`
- support high-quality text, paths, shadows, clipping, and layers through a well-established graphics stack

Implement:
- define a renderer-facing draw IR that maps cleanly to Skia canvas operations
- likely primitives:
  - quad / rounded quad
  - text run or glyph run
  - path
  - shadow
  - image
  - push layer / pop layer
- translate the finalized UI scene or paint output into Skia canvas calls
- keep the UI architecture unchanged above the renderer boundary

Rules:
- this is a backend choice, not a UI architecture change
- the system should not depend on `wgpu` if another backend is a better fit
- text can stay HarfBuzz-shaped if desired, with glyph runs handed to Skia
- backend-facing primitives should stay renderer-oriented and not leak Skia types upward into the UI phases

Why this is next:
- `skia-safe` may be a better practical fit than hand-rolling a renderer stack if the goal is robust 2D rendering rather than `wgpu` specifically
- it keeps the renderer replaceable while letting the higher-level UI architecture stay the same
- this is especially attractive if Vulkan, OpenGL, Metal, or other native backends are acceptable

### Next Step 10. Evaluate Enum-Based `AnyElement` To Remove Dynamic Dispatch
Goal:
- determine whether `AnyElement` should stay a trait-object wrapper or become a closed enum
- remove dynamic dispatch and trait-object allocation from the hot element path if the element set stays small

Implement:
- prototype an enum-based `AnyElement` for the current hand-written element set
- store `ElementBox<Quad>`, `ElementBox<Label>`, `ElementBox<Button>`, `ElementBox<Div>`, and future leaf types as enum variants
- replace trait-object dispatch with `match`-based dispatch inside `AnyElement`

Why this is next:
- the MVP still needs one uniform element container, but that does not require trait objects if the set of element types is intentionally closed
- an enum-based `AnyElement` may be easier to reason about while the project is still small
- this keeps typed phase state while reducing runtime indirection

Tradeoff to evaluate:
- trait-object `AnyElement` keeps the element set open and extensible
- enum-based `AnyElement` removes dynamic dispatch but requires updating the enum whenever a new element type is added
- the right choice depends on whether this codebase wants openness or concrete simplicity more
