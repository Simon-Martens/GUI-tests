# Rebuild Plan for `ai_generated`

## What Problem We Are Solving
We do not want to build "just enough code to draw a button once". We want a small UI system that stays understandable as soon as we add:
- more than one widget
- persistent widget state
- layout
- hover and click
- clipping

The naive version of immediate mode usually starts like this:
- build widgets directly during the frame
- mix layout decisions with drawing
- inspect mouse position while declaring widgets
- mutate app state inside rendering code

That works for one button. It breaks down quickly.

### Why the naive approach becomes a problem

#### 1. Declaration and geometry are different problems
When we declare:
- "there is a panel"
- "inside it there is a label"
- "inside it there is a button"

we still do not know:
- the final sizes
- the final positions
- the final visible clip

So declaration and geometry cannot be the same stage.

#### 2. Input needs final geometry
Hover and click must be based on the actual on-screen bounds.

That means:
- we must know layout first
- then build hit targets from the real bounds

So input cannot be resolved while the widget is still being declared.

#### 3. Paint should not contain business logic
Painting should answer:
- what rectangles do we draw?
- what text do we draw?

Painting should not answer:
- where should this widget go?
- is this widget hovered?
- should clicking this widget mutate app state right now?

So paint needs already-resolved state.

#### 4. Some state must survive frames
The element tree can be rebuilt every frame.
But some things must persist:
- counters
- previous active button
- last known widget rect
- stable widget identity

So we need retained state separate from frame-local state.

#### 5. The renderer should stay dumb
`wgpu` should not know what a button or a label is.
It should only receive:
- rectangles
- text

So the UI system must stop at a draw-command boundary.

### Why we need the layers

#### Retained root view
We need one retained place for application state and frame-to-frame logic.
This is the role of `Render`.

#### Fresh element tree each frame
We want the immediate-mode convenience of rebuilding the UI every frame.
This is the role of `AnyElement` and the element tree.

#### Layout stage
We need a stage that only answers:
- how big is each element?
- where is each element?

This is the role of `request_layout` and `taffy`.

#### Prepaint stage
We need a stage that only answers:
- what are this frame's hitboxes?
- what is the current content mask?
- what geometry should be retained for widget bookkeeping?

This is the role of `prepaint`.

#### Interaction resolution stage
We need a stage that only answers:
- which widget is hovered?
- which widget is active?
- which widget was clicked?
- which actions should be emitted?

This happens after prepaint and before paint.

#### Paint stage
We need a stage that only answers:
- what draw commands should we emit?

This is the role of `paint`.

#### Renderer stage
We need a final stage that only answers:
- how do we turn draw commands into GPU draw calls?

This is the role of `gpu.rs`.

## Final Constraints
- Keep `winit`.
- Keep `wgpu`.
- Keep FreeType + HarfBuzz.
- Use `taffy`.
- Use `euclid`.
- Use `euclid` geometry types directly wherever practical:
  - `Point2D`
  - `Vector2D`
  - `Size2D`
  - `Box2D` or `Rect`
- Only add thin local aliases or helpers in `geom.rs` when they remove friction.
- No images.
- No keyboard input.
- No text input.
- No text selection.
- No scrolling.
- No focus system.
- No drag and drop.
- Only support:
  - rectangles/quads
  - text
  - buttons
  - vertical containers
- Only support:
  - hover
  - click

## Final Target Shape
The final system should look like this:

1. retained root view implements `Render`
2. each frame `render()` builds a fresh `AnyElement` tree
3. root enters:
   - `request_layout`
   - `prepaint`
   - frame interaction resolution
   - `paint`
4. paint emits only draw commands
5. renderer consumes only draw commands

## Working Style For This Rebuild
This must be rebuilt like a human would:
- start from the smallest working program
- keep every step runnable
- add one concept at a time
- do not introduce abstractions before they solve a concrete problem

That means each stage below ends in a working state.

## File Layout
- `src/main.rs`
- `src/app.rs`
- `src/geom.rs`
- `src/gpu.rs`
- `src/text.rs`
- `src/ui.rs`

## Dependencies
Add these from the start:
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
Goal:
- set up the project with the final file layout

Status:
- complete

Do:
1. Create the crate.
2. Add the dependencies.
3. Add the module files.
4. Wire module declarations in `main.rs`.

Working state:
- `cargo check` passes

Why this stage exists:
- remove dependency and file-layout uncertainty before building abstractions

### Stage 1. Window + GPU + One Rectangle
Goal:
- have the smallest visible working program

Status:
- complete

Implement:
- `geom.rs`
  - `Point`
  - `Vec2`
  - `Size`
  - `Rect` backed by `euclid` (`Box2D` or `Rect`, whichever fits the stage better)
  - `Color`
  - `rgb`
  - `to_ndc`
  - only minimal helper constructors/conversions
- `gpu.rs`
  - `DrawCmd::Rect`
  - `GpuState`
  - simple WGSL shader
  - rect tessellation
- `app.rs`
  - `winit` app shell
  - one redraw loop

Use `main.rs` to draw:
- one hardcoded background rectangle
- one smaller colored rectangle

Working state:
- a window opens
- two rectangles render
- resizing still works

Why this stage exists:
- prove the platform + GPU foundation before adding text or UI abstractions

Do not add yet:
- text
- UI memory
- elements
- layout

### Stage 2. Add Text, Still Without UI Abstractions
Goal:
- support the second final paint primitive before any UI architecture exists

Status:
- complete

Implement:
- `text.rs`
  - text context
  - font loading
  - `measure`
  - `rasterize`
- extend `gpu.rs`
  - `DrawCmd::Text`
  - text tessellation from rasterized glyph pixels

Use `main.rs` to draw:
- the same rectangles
- one line of text

Working state:
- text appears
- `measure()` returns usable sizes

Why this stage exists:
- text is a final rendering primitive
- it should be solved before layout or widgets depend on it

Do not add yet:
- retained UI state
- element tree
- interaction

### Stage 3. Add Retained Frame State, Still Without Elements
Goal:
- introduce persistent state in the smallest possible form

Status:
- complete

Implement in `ui.rs`:
- `UiMemory`
  - `frame`
  - `hovered`
  - `active`
  - `ints`
  - `widgets`
- `WidgetState`
  - `id`
  - `last_touched_frame`
  - `last_rect`

Add methods:
- `begin_frame`
- `end_frame`
- `bump`
- `get_int`

Working state:
- the app can persist a counter across frames
- even if the UI system is not real yet

Why this stage exists:
- persistent state is conceptually different from the frame tree
- get that separation right before building widgets

Do not add yet:
- `Render`
- `Element`
- `AnyElement`

### Stage 4. Add a Root `Render` View
Goal:
- separate retained app/view state from frame-local drawing

Status:
- complete

Implement in `ui.rs`:
- `Render`
  - `fn render(&mut self, window: &mut Window<'_>) -> AnyElement;`

Implement in `app.rs`:
- `run<V: Render>(view: V)`
- keep the view object retained in `App`

For now:
- `AnyElement` can be a temporary placeholder if needed
- `Window` can be incomplete

Working state:
- the app owns a retained root view object

Why this stage exists:
- this is the place where app state lives across frames
- we want "retained root view, fresh frame tree"

### Stage 5. Add the Frame `Window` Object
Goal:
- create the frame-owned state object

Status:
- complete

Implement in `ui.rs`:
- `Window<'a>`
  - references to `UiMemory` and `InputState`
  - `screen_size`
  - `frame`
  - temporary `draw_list`

Add helpers:
- `screen_size()`
- `screen_rect()`
- `frame()`
- `counter(...)`
- `bump_counter_action(...)`

Working state:
- `Render::render()` can query frame info and counters

Why this stage exists:
- we need one object that represents "this frame"
- later it will own layout, hitboxes, actions, and paint output

Do not add yet:
- `taffy`
- element phase traits

### Stage 6. Add a Minimal Element Trait With One Leaf Element
Goal:
- introduce the element abstraction in the smallest possible working form

Status:
- complete

Implement in `ui.rs`:
- `Element`
  - `RequestLayoutState`
  - `PrepaintState`
  - `request_layout`
  - `prepaint`
  - `paint`

Start with one very small concrete element:
- `Quad`

At first, it can be extremely simple:
- fixed bounds
- no input
- paint one rect

Working state:
- the root view can return one element
- that element can flow through `request_layout -> prepaint -> paint`

Why this stage exists:
- this is the architectural center
- get the phase contract right before erasing types or adding containers

### Stage 7. Add `AnyElement`
Goal:
- wrap elements behind `AnyElement` while keeping typed phase state

Status:
- complete

Implement:
- `AnyElement`
- internal `GenericElement` trait
- internal typed storage per concrete element

`AnyElement` must store:
- the concrete element
- request-layout state
- prepaint state
- resolved bounds
- node id

Working state:
- the root view can return `AnyElement`
- the phase state stays attached to the element instance

Why this stage exists:
- this is what replaces parallel `layout_nodes` and `prepaint_nodes`

Do not allow:
- a global vector of layout states keyed by element index
- a global vector of prepaint states keyed by element index

### Stage 8. Add Root Phase Driving
Goal:
- make the root use the same element pipeline as everything else

Status:
- complete

Implement on `AnyElement`:
- `prepaint_as_root(...)`
- `paint(...)`

For now:
- use a trivial root layout path if needed
- `taffy` can be added next

Working state:
- root view builds one root element
- root element can be prepainted and painted through the same abstraction as children

Why this stage exists:
- root special-casing is a common source of architectural drift

### Stage 9. Add `taffy`
Goal:
- replace any temporary manual positioning with the actual layout engine

Extend `Window`:
- add `taffy`

Update `AnyElement::prepaint_as_root(...)`:
- create a root layout node
- run `compute_layout`

Update the current elements:
- `Quad`
- any text element

Working state:
- one or two simple elements can be laid out through `taffy`

Why this stage exists:
- layout is a separate problem from paint
- we want to solve it once, centrally

### Stage 10. Add `IntoElement` and `ParentElement`
Goal:
- make composition ergonomic only after the underlying element model works

Implement:
- `IntoElement`
- `ParentElement`

Working state:
- a container can accept children fluently

Why this stage exists:
- builder ergonomics should come after the phase model works

### Stage 11. Add `Div` as the First Real Container
Goal:
- add one general container that supports most of the minimal UI

Implement `Div` with:
- optional id
- optional absolute position
- optional fixed size
- padding
- gap
- optional background
- clip flag
- block_mouse flag
- `Vec<AnyElement>` children

Implement:
- `request_layout`
- `prepaint`
- `paint`

Working state:
- one vertical panel can contain labels and buttons later

Why this stage exists:
- composition must be element-owned
- we want one real container before more leaf widgets

### Stage 12. Add Absolute Text
Goal:
- add text as a leaf element in the element tree

Implement:
- `AbsoluteText`

Working state:
- root `Div` + absolute text works

Why this stage exists:
- this gives us a second leaf type and proves the element system is not hardcoded to rectangles

### Stage 13. Add Flow `Label`
Goal:
- add a text element that participates in container layout

Implement:
- `Label`

It should:
- measure itself during `request_layout`
- paint at resolved bounds

Working state:
- a panel with one or more labels lays out correctly

Why this stage exists:
- this is the first real proof that layout and paint are correctly separated

### Stage 14. Add Button Layout and Paint, But No Input Yet
Goal:
- add button visuals and intrinsic sizing before interaction

Implement:
- `Button`
  - local id
  - label
  - scale
  - padding
  - optional action
- `ButtonRequestLayoutState`
  - measured text size

At this step:
- button paints as a normal non-interactive widget

Working state:
- a panel with label + button lays out and renders correctly

Why this stage exists:
- button geometry is a layout problem
- button interaction is a separate later problem

### Stage 15. Add Stable IDs
Goal:
- support stable widget identity across frames

Implement:
- `LocalElementId`
- `GlobalElementId`
- id hashing/scoping

Use ids on:
- containers that need retained state
- buttons

Working state:
- widgets have stable ids across frames

Why this stage exists:
- hover/active/counters all need stable identity

### Stage 16. Add Hitboxes
Goal:
- introduce frame-local interaction primitives before resolving interaction

Implement:
- `Hitbox`
  - `id`
  - `rect`
  - `content_mask`
  - `behavior`
  - `on_click`
- `HitboxBehavior`
  - `Clickable`
  - `BlockMouse`

Add helpers on `Window`:
- `push_clickable_hitbox`
- `push_blocking_hitbox`

Working state:
- buttons can register hitboxes during prepaint
- blocking panels/quads can block input behind them

Why this stage exists:
- interaction needs a current-frame geometry structure
- this structure should exist before hover/click logic is added

### Stage 17. Add Content Masks
Goal:
- make clipping and hit testing agree

Implement on `Window`:
- `content_masks`
- `push_content_mask`
- `pop_content_mask`
- `current_content_mask`

Use masks in:
- `prepaint`
- `paint`
- hit testing

Working state:
- clipped children do not paint or receive hover outside their visible area

Why this stage exists:
- otherwise paint and interaction diverge immediately

### Stage 18. Add Frame Interaction Resolution
Goal:
- resolve hover/active/clicked after the frame is prepainted

Implement:
- `FrameInteraction`
- `hit_test`
- `resolve_interaction`

Semantics:
- `hovered`: topmost clickable hitbox under cursor after mask clipping
- `active`: widget captured on mouse press
- `clicked`: `hovered == previous active` on mouse release

Working state:
- hover and click are resolved from current-frame geometry

Why this stage exists:
- input must happen after layout and prepaint, never during declaration

### Stage 19. Add Button Interaction
Goal:
- make the button the first fully interactive widget

Extend `Button`:
- `ButtonPrepaintState`
- register clickable hitbox in `prepaint`
- read `Window` interaction in `paint`

Working state:
- button hover changes visuals
- button click can emit an action

Why this stage exists:
- this is the first proof that the whole pipeline works

### Stage 20. Add Actions and `UiOutput`
Goal:
- separate UI event production from app-state mutation

Implement:
- `UiAction`
- `UiOutput`
  - `draw_list`
  - `actions`
  - `interaction`

Rules:
- elements emit actions
- app applies actions later
- no app-state mutation inside element paint

Working state:
- clicking the button emits an action, but UI does not directly mutate app state

Why this stage exists:
- it keeps UI rendering code from turning into game logic

### Stage 21. Add `Window::draw`
Goal:
- create the full frame driver

Implement this order:
1. clear frame-local collections
2. call `render()`
3. root `prepaint_as_root(...)`
4. resolve interaction
5. root `paint(...)`
6. return `UiOutput`

Working state:
- full UI pipeline runs end to end

Why this stage exists:
- the whole architecture only becomes real when the frame order is explicit

### Stage 22. Wire the App Shell to the New UI Pipeline
Goal:
- stop drawing manually from `main.rs`
- let the retained root view drive the frame

In `app.rs`:
1. create `Window`
2. call `window.draw(&mut root_view)`
3. apply actions to retained app/UI state
4. render the returned draw list

Working state:
- frame production and frame rendering are cleanly separated

Why this stage exists:
- this is the integration boundary between UI and app shell

### Stage 23. Build the Minimal Demo
Goal:
- prove the final minimal viable system

In `main.rs`, implement a retained demo view that returns:
- full-screen background `div`
- one absolute `quad`
- one absolute text line
- one centered panel `div`
- one `label`
- one `button`

The button should bump a retained counter.

Working state:
- the system looks like a real minimal UI demo

Why this stage exists:
- this is the smallest demo that exercises:
  - layout
  - text
  - retained state
  - hover
  - click
  - actions
  - paint

## Runtime Validation Checklist
Run these checks once the full pipeline is wired:

1. Window opens and redraws.
2. Background renders.
3. Test rectangle renders.
4. Text renders.
5. Panel sizes from children under `taffy`.
6. Hovering the button changes button color.
7. Press inside / release inside the button triggers exactly once.
8. Press inside / release outside the button does not trigger.
9. Blocking rectangles or containers prevent hover/click behind them.
10. Clipped children do not paint outside their mask.
11. Clipped children do not receive hover/click outside their mask.
12. Counter persists across frames.

## Explicitly Not Allowed
- no double-build frame hack
- no `button() -> bool` API that depends on a second pass
- no global element tree plus parallel layout/prepaint side arrays
- no layout decisions in paint
- no hit testing based on previous-frame rects
- no root-only phase path that bypasses the element abstraction
- no app-state mutation inside paint

## Minimum Viable Completion Criteria
The rebuild is minimally viable when:
- root view implements `Render`
- root returns an `AnyElement` tree
- root and children use the same `Element` contract
- `request_layout` returns typed layout state
- `prepaint` returns typed prepaint state
- `taffy` is the only layout engine
- interaction is resolved from current-frame hitboxes after prepaint
- buttons register click metadata during prepaint
- paint emits only rect/text draw commands
- renderer consumes only draw commands
- app applies emitted actions after the frame is produced



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
