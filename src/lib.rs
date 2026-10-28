//! Small AngelCode BMFont text-format renderer for Macroquad.
//! Supports one-page text-format .fnt files, glyph metrics and kerning.
//! Binary BMFont, packed channels, multiple pages and advanced shaping are out of scope.

use std::collections::HashMap;
use macroquad::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub x: i32, pub y: i32, pub width: i32, pub height: i32,
    pub x_offset: i32, pub y_offset: i32, pub x_advance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSize { pub width: f32, pub height: f32 }

/// A loaded single-page AngelCode BMFont.
pub struct BmFont {
    texture: Texture2D,
    glyphs: HashMap<u32, Glyph>,
    kernings: HashMap<(u32, u32), i32>,
    pub line_height: f32,
    pub base: f32,
}

impl BmFont {
    /// Load a text-format .fnt file and its atlas page, relative to the .fnt file.
    pub async fn load(path: &str) -> Result<Self, String> {
        let text = load_string(path).await.map_err(|e| format!("load {path}: {e}"))?;
        let page = parse_page_file(&text)?;
        let parent = std::path::Path::new(path).parent().unwrap_or_else(|| std::path::Path::new(""));
        let atlas_path = parent.join(page).to_string_lossy().replace('\\', "/");
        let texture = load_texture(&atlas_path).await
            .map_err(|e| format!("load BMFont atlas {atlas_path}: {e}"))?;
        texture.set_filter(FilterMode::Nearest);
        Self::from_fnt_str(&text, texture)
    }

    /// Build a font from BMFont text and an already-loaded atlas texture.
    pub fn from_fnt_str(text: &str, texture: Texture2D) -> Result<Self, String> {
        let mut line_height = None;
        let mut base = None;
        let mut glyphs = HashMap::new();
        let mut kernings = HashMap::new();

        for (line_no, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let (kind, attrs) = parse_line(line);
            match kind.as_str() {
                "common" => {
                    line_height = get_i32(&attrs, "lineHeight").map(|v| v as f32);
                    base = get_i32(&attrs, "base").map(|v| v as f32);
                }
                "char" => {
                    let required = |key: &str| get_i32(&attrs, key)
                        .ok_or_else(|| format!("BMFont line {}: missing/invalid {key}", line_no + 1));
                    let id = required("id")? as u32;
                    let glyph = Glyph {
                        x: required("x")?, y: required("y")?,
                        width: required("width")?, height: required("height")?,
                        x_offset: required("xoffset")?, y_offset: required("yoffset")?,
                        x_advance: required("xadvance")?,
                    };
                    if glyph.x < 0 || glyph.y < 0 || glyph.width < 0 || glyph.height < 0 {
                        return Err(format!("BMFont line {}: negative atlas coordinates or glyph size", line_no + 1));
                    }
                    glyphs.insert(id, glyph);
                }
                "kerning" => {
                    if let (Some(first), Some(second), Some(amount)) =
                        (get_i32(&attrs, "first"), get_i32(&attrs, "second"), get_i32(&attrs, "amount")) {
                        kernings.insert((first as u32, second as u32), amount);
                    }
                }
                _ => {}
            }
        }
        let line_height = line_height.ok_or("BMFont is missing a valid common lineHeight")?;
        let base = base.ok_or("BMFont is missing a valid common base")?;
        if line_height <= 0.0 { return Err("BMFont lineHeight must be positive".into()); }
        if glyphs.is_empty() { return Err("BMFont contains no char entries".into()); }
        texture.set_filter(FilterMode::Nearest);
        Ok(Self { texture, glyphs, kernings, line_height, base })
    }

    pub fn glyph(&self, character: char) -> Option<&Glyph> {
        self.glyphs.get(&(character as u32))
    }

    /// Draw text with (x, baseline_y) as its origin and uniform scale/tint.
    pub fn draw_text(&self, text: &str, x: f32, baseline_y: f32, scale: f32, tint: Color) {
        if scale <= 0.0 { return; }
        let mut pen_x = x;
        let mut baseline = baseline_y;
        let mut previous: Option<u32> = None;
        for ch in text.chars() {
            if ch == '\n' {
                pen_x = x;
                baseline += self.line_height * scale;
                previous = None;
                continue;
            }
            let id = ch as u32;
            let Some(glyph) = self.glyphs.get(&id).or_else(|| self.glyphs.get(&(b'?' as u32))) else {
                previous = Some(id);
                continue;
            };
            if let Some(prev) = previous {
                pen_x += *self.kernings.get(&(prev, id)).unwrap_or(&0) as f32 * scale;
            }
            if glyph.width > 0 && glyph.height > 0 {
                draw_texture_ex(
                    &self.texture,
                    pen_x + glyph.x_offset as f32 * scale,
                    baseline - self.base * scale + glyph.y_offset as f32 * scale,
                    tint,
                    DrawTextureParams {
                        dest_size: Some(vec2(glyph.width as f32 * scale, glyph.height as f32 * scale)),
                        source: Some(Rect::new(glyph.x as f32, glyph.y as f32, glyph.width as f32, glyph.height as f32)),
                        ..Default::default()
                    },
                );
            }
            pen_x += glyph.x_advance as f32 * scale;
            previous = Some(id);
        }
    }

    /// Measure advance width and line-box height at the requested scale.
    pub fn measure_text(&self, text: &str, scale: f32) -> TextSize {
        if scale <= 0.0 || text.is_empty() {
            return TextSize { width: 0.0, height: 0.0 };
        }
        let mut line_width = 0.0_f32;
        let mut max_width = 0.0_f32;
        let mut lines = 1_u32;
        let mut previous: Option<u32> = None;
        for ch in text.chars() {
            if ch == '\n' {
                max_width = max_width.max(line_width);
                line_width = 0.0;
                lines += 1;
                previous = None;
                continue;
            }
            let id = ch as u32;
            if let Some(glyph) = self.glyphs.get(&id).or_else(|| self.glyphs.get(&(b'?' as u32))) {
                if let Some(prev) = previous {
                    line_width += *self.kernings.get(&(prev, id)).unwrap_or(&0) as f32;
                }
                line_width += glyph.x_advance as f32;
                previous = Some(id);
            } else {
                previous = Some(id);
            }
        }
        max_width = max_width.max(line_width);
        TextSize { width: max_width * scale, height: self.line_height * lines as f32 * scale }
    }
}

fn parse_page_file(text: &str) -> Result<String, String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("page ") {
            let (_, attrs) = parse_line(line);
            return attrs.get("file").cloned().ok_or_else(|| "BMFont page entry is missing file".into());
        }
    }
    Err("BMFont has no page entry (expected a single atlas page)".into())
}

fn parse_line(line: &str) -> (String, HashMap<String, String>) {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !token.is_empty() { tokens.push(std::mem::take(&mut token)); }
            }
            c => token.push(c),
        }
    }
    if !token.is_empty() { tokens.push(token); }
    let kind = tokens.first().cloned().unwrap_or_default();
    let mut attrs = HashMap::new();
    for item in tokens.iter().skip(1) {
        if let Some((key, value)) = item.split_once('=') {
            attrs.insert(key.to_owned(), value.to_owned());
        }
    }
    (kind, attrs)
}

fn get_i32(attrs: &HashMap<String, String>, key: &str) -> Option<i32> {
    attrs.get(key)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    const FONT: &str = r#"
info face="Test Font" size=8
common lineHeight=10 base=8 scaleW=16 scaleH=16 pages=1 packed=0
page id=0 file="font atlas.png"
chars count=2
char id=65 x=0 y=0 width=5 height=7 xoffset=0 yoffset=1 xadvance=6 page=0 chnl=0
char id=86 x=5 y=0 width=5 height=7 xoffset=0 yoffset=1 xadvance=6 page=0 chnl=0
kernings count=1
kerning first=65 second=86 amount=-1
"#;

    #[test]
    fn parses_page_glyphs_and_kerning() {
        let (kind, attrs) = parse_line("page id=0 file=\"font atlas.png\"");
        assert_eq!(kind, "page");
        assert_eq!(attrs.get("file").unwrap(), "font atlas.png");
        let (_, common) = parse_line(FONT.lines().find(|l| l.trim().starts_with("common ")).unwrap().trim());
        assert_eq!(get_i32(&common, "lineHeight"), Some(10));
        let mut glyph_count = 0;
        let mut kerning = None;
        for line in FONT.lines() {
            let (kind, attrs) = parse_line(line.trim());
            if kind == "char" { glyph_count += 1; }
            if kind == "kerning" { kerning = get_i32(&attrs, "amount"); }
        }
        assert_eq!(glyph_count, 2);
        assert_eq!(kerning, Some(-1));
    }
}
