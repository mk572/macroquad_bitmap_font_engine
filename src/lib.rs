//! Small AngelCode BMFont text-format renderer for Macroquad.
//! Supports one-page text-format .fnt files, glyph metrics and kerning.
//! Binary BMFont, packed channels, multiple pages and advanced shaping are out of scope.
//!
//! Parsing and layout live in [`data`] and need no window, so they can be tested on real
//! `.fnt` files; [`BmFont`] adds the atlas texture and drawing.

pub mod data;

use macroquad::prelude::*;

pub use data::{Align, FontData, FontError, Glyph, Layout};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSize {
    pub width: f32,
    pub height: f32,
}

/// How `BmFont::draw_text_ex` draws a string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextParams {
    /// Uniform scale relative to the font's own pixels.
    pub scale: f32,
    pub tint: Color,
    /// Alignment of each line against the x passed to `draw_text_ex`.
    pub align: Align,
    /// A second copy drawn first, moved by this many screen pixels, in this colour.
    pub shadow: Option<(Vec2, Color)>,
    /// A one-font-pixel outline in this colour (eight copies around the text).
    pub outline: Option<Color>,
    /// Round glyph positions to whole screen pixels, so pixel fonts do not shimmer.
    pub snap: bool,
}

impl Default for TextParams {
    fn default() -> Self {
        Self {
            scale: 1.0,
            tint: WHITE,
            align: Align::Left,
            shadow: None,
            outline: None,
            snap: true,
        }
    }
}

/// A loaded single-page AngelCode BMFont.
pub struct BmFont {
    texture: Texture2D,
    data: FontData,
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
        Self {
            line_height: data.line_height,
            base: data.base,
            texture,
            data,
        }
    }

    /// The parsed metrics, glyphs and kerning.
    pub fn data(&self) -> &FontData {
        &self.data
    }

    pub fn glyph(&self, character: char) -> Option<&Glyph> {
        self.data.glyph(character)
    }

    /// Draw text with (x, baseline_y) as its origin and uniform scale/tint.
    pub fn draw_text(&self, text: &str, x: f32, baseline_y: f32, scale: f32, tint: Color) {
        let params = TextParams {
            scale,
            tint,
            ..Default::default()
        };
        self.draw_text_ex(text, x, baseline_y, &params);
    }

    /// Draw text with alignment, shadow, outline and pixel snapping; see [`TextParams`].
    pub fn draw_text_ex(&self, text: &str, x: f32, baseline_y: f32, params: &TextParams) {
        if params.scale <= 0.0 {
            return;
        }
        let layout = self.data.layout(text);
        if let Some((offset, color)) = params.shadow {
            self.draw_layout(&layout, x + offset.x, baseline_y + offset.y, params, color);
        }
        if let Some(color) = params.outline {
            let step = params.scale;
            for (dx, dy) in [
                (-1.0, -1.0),
                (0.0, -1.0),
                (1.0, -1.0),
                (-1.0, 0.0),
                (1.0, 0.0),
                (-1.0, 1.0),
                (0.0, 1.0),
                (1.0, 1.0),
            ] {
                self.draw_layout(
                    &layout,
                    x + dx * step,
                    baseline_y + dy * step,
                    params,
                    color,
                );
            }
        }
        self.draw_layout(&layout, x, baseline_y, params, params.tint);
    }

    fn draw_layout(
        &self,
        layout: &Layout,
        x: f32,
        baseline_y: f32,
        params: &TextParams,
        tint: Color,
    ) {
        let scale = params.scale;
        let snap = |v: f32| if params.snap { v.round() } else { v };
        for placed in &layout.glyphs {
            let glyph = &placed.glyph;
            if glyph.width <= 0 || glyph.height <= 0 {
                continue;
            }
            let line_x = x + params.align.shift(layout.line_widths[placed.line]) * scale;
            let top = baseline_y + placed.line as f32 * self.line_height * scale
                - self.base * scale
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
                        glyph.x as f32,
                        glyph.y as f32,
                        glyph.width as f32,
                        glyph.height as f32,
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
