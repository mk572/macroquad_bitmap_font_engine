//! Small AngelCode BMFont text-format renderer for Macroquad.
//! Supports one-page text-format .fnt files, glyph metrics and kerning.
//! Binary BMFont, packed channels, multiple pages and advanced shaping are out of scope.
//!
//! Parsing and layout live in [`data`] and need no window, so they can be tested on real
//! `.fnt` files; [`BmFont`] adds the atlas texture and drawing.
//!
//! **Colour sets.** An atlas may stack several copies of the glyph grid, each in other colours
//! (the .fnt describes the first; the texture is a multiple of `scaleH` tall). [`Ink`] picks one
//! by number or by the colour it is nearest to, or tints the lightest one. **Larger atlases.** A
//! texture that is a whole multiple of `scaleW` wide is the same font drawn larger; the glyph
//! rectangles are scaled to match.

pub mod data;

use std::cell::OnceCell;

use macroquad::prelude::*;

pub use data::{Align, FontData, FontError, Glyph, Layout, OutlineShape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSize {
    pub width: f32,
    pub height: f32,
}

/// Where the colour of the letters comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ink {
    /// The lightest colour set multiplied by this colour (any colour, alpha included). For a
    /// one-set font, simply a tint.
    Tint(Color),
    /// One colour set by number, in its own colours.
    Set(usize),
    /// The colour set whose average colour is closest to this colour, in its own colours.
    Nearest(Color),
}

/// A copy drawn first, moved by `offset` font pixels (times the scale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset: Vec2,
    pub color: Color,
}

/// Copies drawn around the text, one font pixel (times the scale) away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    pub color: Color,
    pub shape: OutlineShape,
}

/// How `BmFont::draw_text_ex` draws a string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextParams {
    /// Uniform scale relative to the font's own pixels.
    pub scale: f32,
    pub ink: Ink,
    /// Alignment of each line against the x passed to `draw_text_ex`.
    pub align: Align,
    pub shadow: Option<Shadow>,
    /// Drawn after the shadow and before the text.
    pub outline: Option<Outline>,
    /// Round glyph positions to whole screen pixels, so pixel fonts do not shimmer.
    pub snap: bool,
}

impl Default for TextParams {
    fn default() -> Self {
        Self {
            scale: 1.0,
            ink: Ink::Tint(WHITE),
            align: Align::Left,
            shadow: None,
            outline: None,
            snap: true,
        }
    }
}

/// The average colour of each colour set, read back from the texture on first use.
struct SetColours {
    averages: Vec<[f32; 3]>,
    lightest: usize,
}

/// A loaded single-page AngelCode BMFont.
pub struct BmFont {
    texture: Texture2D,
    data: FontData,
    /// Texture pixels per font pixel (1 unless the atlas is drawn larger).
    ratio: f32,
    /// How many colour sets the texture stacks.
    sets: usize,
    set_colours: OnceCell<SetColours>,
    pub line_height: f32,
    pub base: f32,
}

impl BmFont {
    /// Load a text-format .fnt file and its atlas page, relative to the .fnt file.
    pub async fn load(path: &str) -> Result<Self, FontError> {
        let load_error = |what: &str, path: &str, e: macroquad::Error| FontError::Load {
            path: path.to_string(),
            message: format!("{what}: {e}"),
        };
        let text = load_string(path)
            .await
            .map_err(|e| load_error("font file", path, e))?;
        let data = FontData::parse(&text)?;
        let parent = std::path::Path::new(path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        let atlas_path = parent.join(&data.page).to_string_lossy().replace('\\', "/");
        let texture = load_texture(&atlas_path)
            .await
            .map_err(|e| load_error("atlas", &atlas_path, e))?;
        Ok(Self::from_data(data, texture))
    }

    /// Build a font from BMFont text and the atlas PNG's bytes (for example `include_bytes!`).
    pub fn from_bytes(fnt_text: &str, atlas_png: &[u8]) -> Result<Self, FontError> {
        let data = FontData::parse(fnt_text)?;
        let texture = Texture2D::from_file_with_format(atlas_png, Some(ImageFormat::Png));
        Ok(Self::from_data(data, texture))
    }

    /// Build a font from BMFont text and an already-loaded atlas texture.
    pub fn from_fnt_str(text: &str, texture: Texture2D) -> Result<Self, FontError> {
        Ok(Self::from_data(FontData::parse(text)?, texture))
    }

    /// Build a font from parsed data and an atlas texture; the atlas goes to nearest filtering.
    pub fn from_data(data: FontData, texture: Texture2D) -> Self {
        texture.set_filter(FilterMode::Nearest);
        let ratio = if data.scale_w > 0 {
            (texture.width() / data.scale_w as f32).max(1.0)
        } else {
            1.0
        };
        let sets = if data.scale_h > 0 {
            (texture.height() / (data.scale_h as f32 * ratio))
                .round()
                .max(1.0) as usize
        } else {
            1
        };
        Self {
            line_height: data.line_height,
            base: data.base,
            texture,
            data,
            ratio,
            sets,
            set_colours: OnceCell::new(),
        }
    }

    /// The parsed metrics, glyphs and kerning.
    pub fn data(&self) -> &FontData {
        &self.data
    }

    pub fn glyph(&self, character: char) -> Option<&Glyph> {
        self.data.glyph(character)
    }

    /// How many colour sets the atlas holds (1 for an ordinary font).
    pub fn colour_sets(&self) -> usize {
        self.sets
    }

    fn set_colours(&self) -> &SetColours {
        self.set_colours.get_or_init(|| {
            let averages = if self.sets > 1 {
                let image = self.texture.get_texture_data();
                data::set_averages(&image.bytes, image.width(), image.height(), self.sets)
            } else {
                vec![[1.0; 3]]
            };
            SetColours {
                lightest: data::lightest_set(&averages),
                averages,
            }
        })
    }

    /// The colour set whose average colour is closest to `color`.
    pub fn nearest_set(&self, color: Color) -> usize {
        if self.sets <= 1 {
            return 0;
        }
        data::nearest_set(&self.set_colours().averages, [color.r, color.g, color.b])
    }

    /// The set to draw and the tint to multiply it by for an ink.
    fn resolve(&self, ink: Ink) -> (usize, Color) {
        match ink {
            Ink::Set(n) => (n.min(self.sets - 1), WHITE),
            Ink::Nearest(color) => (self.nearest_set(color), WHITE),
            Ink::Tint(color) => (self.set_colours().lightest, color),
        }
    }

    /// Draw text with (x, baseline_y) as its origin and uniform scale/tint.
    pub fn draw_text(&self, text: &str, x: f32, baseline_y: f32, scale: f32, tint: Color) {
        let params = TextParams {
            scale,
            ink: Ink::Tint(tint),
            ..Default::default()
        };
        self.draw_text_ex(text, x, baseline_y, &params);
    }

    /// Draw text with ink, alignment, shadow, outline and pixel snapping; see [`TextParams`].
    pub fn draw_text_ex(&self, text: &str, x: f32, baseline_y: f32, params: &TextParams) {
        if params.scale <= 0.0 {
            return;
        }
        let layout = self.data.layout(text);
        let origin = vec2(x, baseline_y);
        let scale = params.scale;
        if let Some(shadow) = params.shadow {
            let (set, tint) = self.resolve(Ink::Tint(shadow.color));
            self.draw_layout(&layout, origin + shadow.offset * scale, params, set, tint);
        }
        if let Some(outline) = params.outline {
            let (set, tint) = self.resolve(Ink::Tint(outline.color));
            for &(dx, dy) in outline.shape.offsets() {
                self.draw_layout(&layout, origin + vec2(dx, dy) * scale, params, set, tint);
            }
        }
        let (set, tint) = self.resolve(params.ink);
        self.draw_layout(&layout, origin, params, set, tint);
    }

    fn draw_layout(
        &self,
        layout: &Layout,
        origin: Vec2,
        params: &TextParams,
        set: usize,
        tint: Color,
    ) {
        let scale = params.scale;
        let snap = |v: f32| if params.snap { v.round() } else { v };
        // Where the set starts in the texture, and the texture pixels per font pixel.
        let set_top = set as f32 * self.data.scale_h as f32 * self.ratio;
        let k = self.ratio;
        for placed in &layout.glyphs {
            let glyph = &placed.glyph;
            if glyph.width <= 0 || glyph.height <= 0 {
                continue;
            }
            let line_x = origin.x + params.align.shift(layout.line_widths[placed.line]) * scale;
            let top = origin.y + placed.line as f32 * self.line_height * scale - self.base * scale
                + glyph.y_offset as f32 * scale;
            draw_texture_ex(
                &self.texture,
                snap(line_x + (placed.x + glyph.x_offset as f32) * scale),
                snap(top),
                tint,
                DrawTextureParams {
                    dest_size: Some(vec2(
                        glyph.width as f32 * scale,
                        glyph.height as f32 * scale,
                    )),
                    source: Some(Rect::new(
                        glyph.x as f32 * k,
                        glyph.y as f32 * k + set_top,
                        glyph.width as f32 * k,
                        glyph.height as f32 * k,
                    )),
                    ..Default::default()
                },
            );
        }
    }

    /// Measure advance width and line-box height at the requested scale.
    pub fn measure_text(&self, text: &str, scale: f32) -> TextSize {
        if scale <= 0.0 || text.is_empty() {
            return TextSize {
                width: 0.0,
                height: 0.0,
            };
        }
        let layout = self.data.layout(text);
        TextSize {
            width: layout.width() * scale,
            height: self.line_height * layout.line_widths.len() as f32 * scale,
        }
    }

    /// Break `text` at word gaps so no line is wider than `max_width` screen pixels at `scale`.
    pub fn wrap_text(&self, text: &str, max_width: f32, scale: f32) -> String {
        self.data.wrap(text, max_width / scale.max(f32::EPSILON))
    }
}
