use std::cell::RefCell;
use std::collections::HashMap;

use freetype::Library;
use freetype::face::LoadFlag;
use harfbuzz_rs_now::{Face as HbFace, Font as HbFont, UnicodeBuffer, shape};

use crate::geom::Size;

const FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/gnu-free/FreeSans.otf",
    "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
];

thread_local! {
    static TEXT_CONTEXT: RefCell<Option<TextContext>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct TextLayout {
    pub glyphs: Vec<PositionedGlyph>,
    pub advance: f32,
    pub pixel_height: u32,
}

#[derive(Clone)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub x: f32,
    pub y_offset: f32,
}

#[derive(Clone)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub left: f32,
    pub top: f32,
    pub pixels: Vec<u8>,
}

pub fn measure(text: &str, scale: f32) -> Size {
    if text.is_empty() {
        return Size::new(0.0, 0.0);
    }

    // Dont worry; HarfBuzz has a cache that gets reused every time we measure
    let layout = layout(text, scale);
    measure_layout(&layout)
}

pub fn measure_layout(layout: &TextLayout) -> Size {
    let line_height = with_context(|ctx| ctx.line_height(layout.pixel_height));
    Size::new(layout.advance.max(0.0), line_height)
}

pub fn layout(text: &str, scale: f32) -> TextLayout {
    let pixel_height = scale_to_pixels(scale);
    if text.is_empty() {
        return TextLayout {
            glyphs: Vec::new(),
            advance: 0.0,
            pixel_height,
        };
    }

    with_context(|ctx| ctx.layout(text, pixel_height))
}

pub fn rasterize_glyph(glyph_id: u32, pixel_height: u32) -> GlyphBitmap {
    with_context(|ctx| ctx.load_glyph_bitmap(glyph_id, pixel_height))
}

struct TextContext {
    _library: Library,
    face: freetype::Face,
    font_path: &'static str,
    layout_cache: HashMap<LayoutKey, TextLayout>,
}

#[derive(Hash, Eq, PartialEq)]
struct LayoutKey {
    text: String,
    pixel_height: u32,
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
            layout_cache: HashMap::new(),
        }
    }

    fn layout(&mut self, text: &str, pixel_height: u32) -> TextLayout {
        let key = LayoutKey {
            text: text.to_string(),
            pixel_height,
        };

        // Here's a cache, so we do not have to invoke Harfbuzz on every single GPU render pass
        // especially if the same text is already shaped by HarfBuzz
        // TODO: it's not nice to clone the cache to return it here, baecause cloning allocates.
        // Maybe the cache should live in the parents render function.
        // Important: this caches text runs position-independently, so the same HarfBuzz results
        // can be used in multiple places on the screen, eg. when scrolling etc.
        if let Some(layout) = self.layout_cache.get(&key) {
            return layout.clone();
        }

        let layout = shape_text(self.font_path, text, pixel_height);
        self.layout_cache.insert(key, layout.clone());
        layout
    }

    fn line_height(&mut self, pixel_height: u32) -> f32 {
        self.face
            .set_pixel_sizes(0, pixel_height)
            .expect("set pixel sizes");

        let Some(metrics) = self.face.size_metrics() else {
            return pixel_height as f32;
        };

        let ascent = metrics.ascender as f32 / 64.0;
        let descent = metrics.descender as f32 / 64.0;
        let height = metrics.height as f32 / 64.0;

        height
            .max(ascent - descent)
            .max(pixel_height as f32)
            .max(1.0)
    }

    // Here's a lot of work: the actual loading of the glyphs shape by freetype.
    // Maybe we do not cache here at all, since we cache the results of the whole
    // rasterization process in the render functions, so it's unlikely that this
    // gets called very oftne for the same glyph.
    // Good that we have the font face already preparesd, bad that's only one face.
    fn load_glyph_bitmap(&mut self, glyph_id: u32, pixel_height: u32) -> GlyphBitmap {
        self.face
            .set_pixel_sizes(0, pixel_height)
            .expect("set pixel sizes");
        self.face
            .load_glyph(glyph_id, LoadFlag::RENDER)
            .expect("load glyph");

        let slot = self.face.glyph();
        let bitmap = slot.bitmap();
        let width = bitmap.width().max(0) as usize;
        let rows = bitmap.rows().max(0) as usize;
        let pitch = bitmap.pitch().unsigned_abs() as usize;
        let buffer = bitmap.buffer();
        let mut pixels = vec![0; width * rows];

        for row in 0..rows {
            let src = row * pitch;
            let dst = row * width;
            pixels[dst..dst + width].copy_from_slice(&buffer[src..src + width]);
        }

        GlyphBitmap {
            width: width as u32,
            height: rows as u32,
            left: slot.bitmap_left() as f32,
            top: slot.bitmap_top() as f32,
            pixels,
        }
    }
}

// TODO: pull out HbFace creation and up_em caching, as well as HbFont creation to prevent
// allocations.
// Here a lot of the complexity in this step lies: we invioke HarfBuzz for text shaping at
// certain font sizes. We do not need to cach the glyphs reslt here, because it's already wrapped
// by a cache from the rasterization function above.
fn shape_text(font_path: &str, text: &str, pixel_height: u32) -> TextLayout {
    let hb_face = HbFace::from_file(font_path, 0).expect("failed to load harfbuzz face");
    let upem = hb_face.upem() as f32;
    let font = HbFont::new(hb_face);
    let shaped = shape(&font, UnicodeBuffer::new().add_str(text), &[]);
    let infos = shaped.get_glyph_infos();
    let positions = shaped.get_glyph_positions();
    let units_to_pixels = pixel_height as f32 / upem.max(1.0);

    let mut pen_x = 0.0;
    let mut glyphs = Vec::with_capacity(infos.len());

    for (info, position) in infos.iter().zip(positions.iter()) {
        glyphs.push(PositionedGlyph {
            glyph_id: info.codepoint,
            x: pen_x + position.x_offset as f32 * units_to_pixels,
            y_offset: position.y_offset as f32 * units_to_pixels,
        });
        pen_x += position.x_advance as f32 * units_to_pixels;
    }

    TextLayout {
        glyphs,
        advance: pen_x,
        pixel_height,
    }
}

fn scale_to_pixels(scale: f32) -> u32 {
    (scale * 12.0).ceil().max(1.0) as u32
}
