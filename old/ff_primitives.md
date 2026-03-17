Just as inspirtation, here are 33 drawing primitives FF/servo webrender uses to draw stuff:

# WebRender DisplayItem cheat sheet

## Shared helper structs

### CommonItemProperties
Used by many visible primitives.

- clip_rect: LayoutRect
- clip_chain_id: ClipChainId
- spatial_id: SpatialId
- flags: PrimitiveFlags

### SpaceAndClipInfo
Used by scoped items.

- spatial_id: SpatialId
- clip_chain_id: ClipChainId


---

## 1) Real content display items

### 1. Rectangle(RectangleDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- color: PropertyBinding<ColorF>

### 2. ClearRectangle(ClearRectangleDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect

### 3. HitTest(HitTestDisplayItem)
- rect: LayoutRect
- clip_chain_id: ClipChainId
- spatial_id: SpatialId
- flags: PrimitiveFlags
- tag: ItemTag

### 4. Text(TextDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- font_key: FontInstanceKey
- color: ColorF
- glyph_options: Option<GlyphOptions>
- ref_frame_offset: LayoutVector2D
- IMPLICIT: glyphs: Vec<GlyphInstance>

### 5. Line(LineDisplayItem)
- common: CommonItemProperties
- area: LayoutRect
- orientation: LineOrientation
- wavy_line_thickness: f32
- color: ColorF
- style: LineStyle

LineStyle:
- Solid
- Dotted
- Dashed
- Wavy

### 6. Border(BorderDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- widths: LayoutSideOffsets
- details: BorderDetails

BorderDetails:
- Normal(NormalBorder)
- NinePatch(NinePatchBorder)

NormalBorder:
- top: BorderSide
- right: BorderSide
- bottom: BorderSide
- left: BorderSide
- radius: BorderRadius
- do_aa: bool

BorderSide:
- color: ColorF
- style: BorderStyle

BorderStyle:
- None
- Solid
- Double
- Dotted
- Dashed
- Hidden
- Groove
- Ridge
- Inset
- Outset

NinePatchBorder:
- source: NinePatchBorderSource
- width: SideOffsets2D<f32, LayoutPixel>
- slice: SideOffsets2D<u32, DevicePixel>
- fill: bool
- repeat_horizontal: RepeatMode
- repeat_vertical: RepeatMode
- outset: SideOffsets2D<f32, LayoutPixel>

NinePatchBorderSource:
- Image(ImageKey, ImageRendering)
- Gradient(Gradient)
- RadialGradient(RadialGradient)
- ConicGradient(ConicGradient)

RepeatMode:
- Stretch
- Repeat
- Round
- Space

BorderRadius:
- top_left: LayoutSize
- top_right: LayoutSize
- bottom_left: LayoutSize
- bottom_right: LayoutSize

### 7. BoxShadow(BoxShadowDisplayItem)
- common: CommonItemProperties
- box_bounds: LayoutRect
- offset: LayoutVector2D
- color: ColorF
- blur_radius: f32
- spread_radius: f32
- border_radius: BorderRadius
- clip_mode: BoxShadowClipMode

BoxShadowClipMode:
- None
- Outset
- Inset

### 8. PushShadow(PushShadowDisplayItem)
- space_and_clip: SpaceAndClipInfo
- shadow: Shadow
- should_inflate: bool

Shadow:
- offset: LayoutVector2D
- color: ColorF
- blur_radius: f32

### 9. Gradient(GradientDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- tile_size: LayoutSize
- tile_spacing: LayoutSize
- gradient: Gradient

Gradient:
- start_point: LayoutPoint
- end_point: LayoutPoint
- extend_mode: ExtendMode
- IMPLICIT: stops: Vec<GradientStop>

GradientStop:
- offset: f32
- color: ColorF

ExtendMode:
- Clamp
- Repeat

### 10. RadialGradient(RadialGradientDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- gradient: RadialGradient
- tile_size: LayoutSize
- tile_spacing: LayoutSize

RadialGradient:
- center: LayoutPoint
- radius: LayoutSize
- start_offset: f32
- end_offset: f32
- extend_mode: ExtendMode
- IMPLICIT: stops: Vec<GradientStop>

### 11. ConicGradient(ConicGradientDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- gradient: ConicGradient
- tile_size: LayoutSize
- tile_spacing: LayoutSize

ConicGradient:
- center: LayoutPoint
- angle: f32
- start_offset: f32
- end_offset: f32
- extend_mode: ExtendMode
- IMPLICIT: stops: Vec<GradientStop>

### 12. Image(ImageDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- image_key: ImageKey
- image_rendering: ImageRendering
- alpha_type: AlphaType
- color: ColorF

### 13. RepeatingImage(RepeatingImageDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- stretch_size: LayoutSize
- tile_spacing: LayoutSize
- image_key: ImageKey
- image_rendering: ImageRendering
- alpha_type: AlphaType
- color: ColorF

### 14. YuvImage(YuvImageDisplayItem)
- common: CommonItemProperties
- bounds: LayoutRect
- yuv_data: YuvData
- color_depth: ColorDepth
- color_space: YuvColorSpace
- color_range: ColorRange
- image_rendering: ImageRendering

### 15. BackdropFilter(BackdropFilterDisplayItem)
- common: CommonItemProperties
- IMPLICIT: filters: Vec<FilterOp>
- IMPLICIT: filter_datas: Vec<FilterData>
- IMPLICIT: filter_primitives: Vec<FilterPrimitive>


---

## 2) Clip items

### 16. RectClip(RectClipDisplayItem)
- id: ClipId
- spatial_id: SpatialId
- clip_rect: LayoutRect

### 17. RoundedRectClip(RoundedRectClipDisplayItem)
- id: ClipId
- spatial_id: SpatialId
- clip: ComplexClipRegion

ComplexClipRegion:
- rect: LayoutRect
- radii: BorderRadius
- mode: ClipMode

ClipMode:
- Clip
- ClipOut

### 18. ImageMaskClip(ImageMaskClipDisplayItem)
- id: ClipId
- spatial_id: SpatialId
- image_mask: ImageMask
- fill_rule: FillRule
- IMPLICIT: points: Vec<LayoutPoint>

ImageMask:
- image: ImageKey
- rect: LayoutRect
- repeat: bool

FillRule:
- Nonzero
- Evenodd

### 19. ClipChain(ClipChainItem)
- id: ClipChainId
- parent: Option<ClipChainId>
- IMPLICIT: clip_ids: Vec<ClipId>


---

## 3) Spaces / frames / scopes

### 20. Iframe(IframeDisplayItem)
- bounds: LayoutRect
- clip_rect: LayoutRect
- space_and_clip: SpaceAndClipInfo
- pipeline_id: PipelineId
- ignore_missing_pipeline: bool

### 21. PushReferenceFrame(ReferenceFrameDisplayListItem)
- no fields in the display item payload itself

Related descriptor data lives elsewhere in the spatial tree, not here.

### 22. PushStackingContext(PushStackingContextDisplayItem)
- origin: LayoutPoint
- spatial_id: SpatialId
- snapshot: Option<SnapshotInfo>
- prim_flags: PrimitiveFlags
- ref_frame_offset: LayoutVector2D
- stacking_context: StackingContext

SnapshotInfo:
- key: SnapshotImageKey
- area: LayoutRect
- detached: bool

StackingContext:
- transform_style: TransformStyle
- mix_blend_mode: MixBlendMode
- clip_chain_id: Option<ClipChainId>
- raster_space: RasterSpace
- flags: StackingContextFlags
- IMPLICIT: filters: Vec<FilterOp>
- IMPLICIT: filter_datas: Vec<FilterData>
- IMPLICIT: filter_primitives: Vec<FilterPrimitive>

TransformStyle:
- Flat
- Preserve3D

RasterSpace:
- Screen
- Local(f32)


---

## 4) Marker items for trailing arrays

These are not standalone drawing primitives. They mean “the next item consumes this extra array”.

### 23. SetGradientStops
- payload follows as: Vec<GradientStop>

### 24. SetFilterOps
- payload follows as: Vec<FilterOp>

### 25. SetFilterData
- payload follows as: FilterData

### 26. SetFilterPrimitives
- payload follows as: Vec<FilterPrimitive>

FilterPrimitive:
- kind: FilterPrimitiveKind
- color_space: ColorSpace

FilterPrimitiveKind:
- Identity(IdentityPrimitive)
- Blend(BlendPrimitive)
- Flood(FloodPrimitive)
- Blur(BlurPrimitive)
- Opacity(OpacityPrimitive)
- ColorMatrix(ColorMatrixPrimitive)
- DropShadow(DropShadowPrimitive)
- ComponentTransfer(ComponentTransferPrimitive)
- Offset(OffsetPrimitive)
- Composite(CompositePrimitive)

IdentityPrimitive:
- input: FilterPrimitiveInput

BlendPrimitive:
- input1: FilterPrimitiveInput
- input2: FilterPrimitiveInput
- mode: MixBlendMode

FloodPrimitive:
- color: ColorF

BlurPrimitive:
- input: FilterPrimitiveInput
- width: f32
- height: f32

OpacityPrimitive:
- input: FilterPrimitiveInput
- opacity: PropertyBinding<f32>

ColorMatrixPrimitive:
- input: FilterPrimitiveInput
- matrix: [f32; 20]

DropShadowPrimitive:
- input: FilterPrimitiveInput
- shadow: Shadow

ComponentTransferPrimitive:
- input: FilterPrimitiveInput
- transfer data is stored in FilterData

OffsetPrimitive:
- input: FilterPrimitiveInput
- offset: LayoutVector2D

CompositePrimitive:
- input1: FilterPrimitiveInput
- input2: FilterPrimitiveInput
- operator: CompositeOperator

### 27. SetPoints
- payload follows as: Vec<LayoutPoint>


---

## 5) Scope terminators

### 28. PopReferenceFrame
- no fields

### 29. PopStackingContext
- no fields

### 30. PopAllShadows
- no fields


---

## 6) Retention / reuse / debug

### 31. ReuseItems(ItemKey)
- ItemKey = u16

### 32. RetainedItems(ItemKey)
- ItemKey = u16

### 33. DebugMarker(u32)
- debug marker value: u32
