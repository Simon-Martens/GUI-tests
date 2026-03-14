use std::cell::RefCell;

use freetype::Library;
use freetype::face::LoadFlag;
use harfbuzz_rs_now::{Face as HbFace, Font as HbFont, UnicodeBuffer, shape};

use crate::geom::{Color, Rect, Vec2};

const FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/gnu-free/FreeSans.otf",
    "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
];

thread_local! {
    static TEXT_CONTEXT: RefCell<Option<TextContext>> = const { RefCell::new(None) };
}

pub struct GlyphRect {
    pub rect: Rect,
    pub color: Color,
}

pub fn measure(text: &str, scale: f32) -> Vec2 {
    if text.is_empty() {
        return Vec2::ZERO;
    }

    with_context(|ctx| {
        let pixel_height = scale_to_pixels(scale);
        let layout = shape_text(ctx.font_path, text, pixel_height);
        ctx.face
            .set_pixel_sizes(0, pixel_height.ceil() as u32)
            .expect("set pixel sizes");

        let baseline = pixel_height;
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for glyph in &layout.glyphs {
            ctx.face
                .load_glyph(glyph.glyph_id, LoadFlag::RENDER)
                .expect("load glyph");
            let slot = ctx.face.glyph();
            let bitmap = slot.bitmap();
            let x = glyph.x + slot.bitmap_left() as f32;
            let y = baseline - glyph.y_offset - slot.bitmap_top() as f32;
            let w = bitmap.width() as f32;
            let h = bitmap.rows() as f32;
            if w <= 0.0 || h <= 0.0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }

        if min_x.is_finite() {
            Vec2::new(
                (max_x - min_x).max(0.0),
                (max_y - min_y).max(pixel_height * 0.75),
            )
        } else {
            Vec2::new(layout.advance, pixel_height)
        }
    })
}

pub fn rasterize(text: &str, pos: Vec2, scale: f32, color: Color) -> Vec<GlyphRect> {
    if text.is_empty() {
        return Vec::new();
    }

    with_context(|ctx| {
        let pixel_height = scale_to_pixels(scale);
        let layout = shape_text(ctx.font_path, text, pixel_height);
        ctx.face
            .set_pixel_sizes(0, pixel_height.ceil() as u32)
            .expect("set pixel sizes");

        let baseline = pos.y + pixel_height;
        let mut rects = Vec::new();

        for glyph in &layout.glyphs {
            ctx.face
                .load_glyph(glyph.glyph_id, LoadFlag::RENDER)
                .expect("load glyph");
            let slot = ctx.face.glyph();
            let bitmap = slot.bitmap();
            let left = pos.x + glyph.x + slot.bitmap_left() as f32;
            let top = baseline - glyph.y_offset - slot.bitmap_top() as f32;

            let width = bitmap.width().max(0) as usize;
            let rows = bitmap.rows().max(0) as usize;
            let pitch = bitmap.pitch().unsigned_abs() as usize;
            let buffer = bitmap.buffer();

            for row in 0..rows {
                for col in 0..width {
                    let alpha = buffer[row * pitch + col];
                    if alpha == 0 {
                        continue;
                    }
                    let mut pixel_color = color;
                    pixel_color[3] *= alpha as f32 / 255.0;
                    rects.push(GlyphRect {
                        rect: Rect::from_min_size(
                            Vec2::new(left + col as f32, top + row as f32),
                            Vec2::splat(1.0),
                        ),
                        color: pixel_color,
                    });
                }
            }
        }

        rects
    })
}

struct TextContext {
    _library: Library,
    face: freetype::Face,
    font_path: &'static str,
}

struct ShapedGlyph {
    glyph_id: u32,
    x: f32,
    y_offset: f32,
}

struct ShapedText {
    glyphs: Vec<ShapedGlyph>,
    advance: f32,
}

fn with_context<T>(f: impl FnOnce(&mut TextContext) -> T) -> T {
    TEXT_CONTEXT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let ctx = slot.get_or_insert_with(TextContext::new);
        f(ctx)
    })
}

impl TextContext {
    fn new() -> Self {
        let font_path = FONT_PATHS
            .iter()
            .copied()
            .find(|path| std::path::Path::new(path).exists())
            .expect("font file not found");
        let library = Library::init().expect("init freetype");
        let face = library.new_face(font_path, 0).expect("load font face");
        Self {
            _library: library,
            face,
            font_path,
        }
    }
}

fn shape_text(font_path: &str, text: &str, pixel_height: f32) -> ShapedText {
    let hb_face = HbFace::from_file(font_path, 0).expect("load harfbuzz face");
    let upem = hb_face.upem() as f32;
    let font = HbFont::new(hb_face);
    let shaped = shape(&font, UnicodeBuffer::new().add_str(text), &[]);
    let infos = shaped.get_glyph_infos();
    let positions = shaped.get_glyph_positions();
    let units_to_pixels = pixel_height / upem.max(1.0);

    let mut pen_x = 0.0;
    let mut glyphs = Vec::with_capacity(infos.len());
    for (info, position) in infos.iter().zip(positions.iter()) {
        glyphs.push(ShapedGlyph {
            glyph_id: info.codepoint,
            x: pen_x + position.x_offset as f32 * units_to_pixels,
            y_offset: position.y_offset as f32 * units_to_pixels,
        });
        pen_x += position.x_advance as f32 * units_to_pixels;
    }

    ShapedText {
        glyphs,
        advance: pen_x,
    }
}

fn scale_to_pixels(scale: f32) -> f32 {
    (scale * 12.0).max(1.0)
}
