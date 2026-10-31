//! The parts of a BMFont that need no window: parsing the text format and laying text out.
//! Everything here works in the font's own pixels; `BmFont` scales and draws it.

use std::collections::HashMap;
use std::fmt;

/// One glyph's rectangle in the atlas and how it sits on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub x_advance: i32,
}

/// Why a font could not be loaded or parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The .fnt file or the atlas could not be read or decoded.
    Load { path: String, message: String },
    /// A line has a missing or invalid value.
    Line { line: usize, message: String },
    /// A required entry (`common`, `page`, `char`) is absent.
    Missing(&'static str),
    /// A feature this engine leaves out (binary format, several pages).
    Unsupported(&'static str),
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load { path, message } => write!(f, "load {path}: {message}"),
            Self::Line { line, message } => write!(f, "BMFont line {line}: {message}"),
            Self::Missing(what) => write!(f, "BMFont is missing {what}"),
            Self::Unsupported(what) => write!(f, "BMFont uses {what}, which is not supported"),
        }
    }
}

impl std::error::Error for FontError {}

/// A glyph placed on a line: its pen position (before the glyph's own offset) in font pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub glyph: Glyph,
    pub x: f32,
    pub line: usize,
}

/// Text laid out once, shared by drawing and measuring.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub glyphs: Vec<Placed>,
    /// The advance width of each line, in font pixels.
    pub line_widths: Vec<f32>,
}

impl Layout {
    pub fn width(&self) -> f32 {
        self.line_widths.iter().copied().fold(0.0, f32::max)
    }
}

/// Horizontal alignment of each line against the x passed to `draw_text_ex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

impl Align {
    /// How far a line of `width` starts left of the anchor.
    pub fn shift(self, width: f32) -> f32 {
        match self {
            Self::Left => 0.0,
            Self::Center => -width / 2.0,
            Self::Right => -width,
        }
    }
}

/// Which copies an outline draws around the text, in font pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlineShape {
    /// All eight neighbours.
    #[default]
    Full,
    /// Left, right, up and down.
    Cross,
    /// Left, right, up, down and the lower right: the shape of the original's gold and mana
    /// figures.
    CrossAndLowerRight,
}

impl OutlineShape {
    /// The offsets, in font pixels, of the copies drawn behind the text.
    pub fn offsets(self) -> &'static [(f32, f32)] {
        match self {
            Self::Full => &[
                (-1.0, -1.0),
                (0.0, -1.0),
                (1.0, -1.0),
                (-1.0, 0.0),
                (1.0, 0.0),
                (-1.0, 1.0),
                (0.0, 1.0),
                (1.0, 1.0),
            ],
            Self::Cross => &[(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)],
            Self::CrossAndLowerRight => {
                &[(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (1.0, 1.0)]
            }
        }
    }
}

/// The average colour of each colour set of an atlas that stacks `sets` copies of the glyph
/// grid (RGBA bytes, `width` wide): the mean of the drawn pixels, 0 to 1. A set with no drawn
/// pixel averages to black.
pub fn set_averages(rgba: &[u8], width: usize, height: usize, sets: usize) -> Vec<[f32; 3]> {
    let rows = height / sets.max(1);
    (0..sets)
        .map(|set| {
            let from = set * rows * width * 4;
            let to = (from + rows * width * 4).min(rgba.len());
            let (mut sum, mut n) = ([0.0_f32; 3], 0.0_f32);
            for p in rgba.get(from..to).unwrap_or(&[]).as_chunks::<4>().0.iter() {
                if p[3] != 0 {
                    for (acc, c) in sum.iter_mut().zip(p) {
                        *acc += f32::from(*c) / 255.0;
                    }
                    n += 1.0;
                }
            }
            sum.map(|v| if n > 0.0 { v / n } else { 0.0 })
        })
        .collect()
}

/// The set whose average colour is closest to `color` (red, green, blue, 0 to 1).
pub fn nearest_set(averages: &[[f32; 3]], color: [f32; 3]) -> usize {
    let distance = |a: &[f32; 3]| {
        a.iter()
            .zip(color)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
    };
    averages
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| distance(a).total_cmp(&distance(b)))
        .map_or(0, |(i, _)| i)
}

/// The set with the lightest average colour: the one a flat tint multiplies.
pub fn lightest_set(averages: &[[f32; 3]]) -> usize {
    averages
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.iter().sum::<f32>().total_cmp(&b.iter().sum::<f32>()))
        .map_or(0, |(i, _)| i)
}

/// A parsed single-page text BMFont: metrics, glyphs and kerning, no texture.
#[derive(Debug, Clone, PartialEq)]
pub struct FontData {
    pub line_height: f32,
    pub base: f32,
    /// The atlas size the file declares (`common scaleW`, `scaleH`), 0 when it does not say.
    /// An atlas that stacks several colour sets is a multiple of `scale_h` tall; a texture a
    /// multiple of `scale_w` wide is a larger drawing of the same font.
    pub scale_w: u32,
    pub scale_h: u32,
    /// `info padding`: up, right, down, left.
    pub padding: [i32; 4],
    /// `info spacing`: horizontal, vertical.
    pub spacing: [i32; 2],
    /// The atlas file named by the `page` entry.
    pub page: String,
    glyphs: HashMap<u32, Glyph>,
    kernings: HashMap<(u32, u32), i32>,
}

impl FontData {
    pub fn parse(text: &str) -> Result<Self, FontError> {
        if text.starts_with("BMF") {
            return Err(FontError::Unsupported("the binary format"));
        }
        let mut line_height = None;
        let mut base = None;
        let mut atlas = (0, 0);
        let mut page = None;
        let mut padding = [0; 4];
        let mut spacing = [0; 2];
        let mut glyphs = HashMap::new();
        let mut kernings = HashMap::new();

        for (index, line) in text.lines().enumerate() {
            let line_no = index + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (kind, attrs) = parse_line(line);
            let bad = |message: String| FontError::Line {
                line: line_no,
                message,
            };
            let required = |key: &str| {
                get_i32(&attrs, key).ok_or_else(|| bad(format!("missing or invalid {key}")))
            };
            match kind.as_str() {
                "info" => {
                    padding = get_list(&attrs, "padding").unwrap_or(padding);
                    spacing = get_list(&attrs, "spacing").unwrap_or(spacing);
                }
                "common" => {
                    line_height = Some(required("lineHeight")? as f32);
                    base = Some(required("base")? as f32);
                    atlas = (
                        get_i32(&attrs, "scaleW").unwrap_or(0).max(0) as u32,
                        get_i32(&attrs, "scaleH").unwrap_or(0).max(0) as u32,
                    );
                    if get_i32(&attrs, "pages").unwrap_or(1) > 1 {
                        return Err(FontError::Unsupported("several atlas pages"));
                    }
                }
                "page" if page.is_none() => {
                    page = Some(
                        attrs
                            .get("file")
                            .cloned()
                            .ok_or_else(|| bad("page entry is missing file".into()))?,
                    );
                }
                "char" => {
                    let id = required("id")?;
                    let glyph = Glyph {
                        x: required("x")?,
                        y: required("y")?,
                        width: required("width")?,
                        height: required("height")?,
                        x_offset: required("xoffset")?,
                        y_offset: required("yoffset")?,
                        x_advance: required("xadvance")?,
                    };
                    if id < 0 || glyph.x < 0 || glyph.y < 0 || glyph.width < 0 || glyph.height < 0 {
                        return Err(bad("negative id, atlas coordinate or glyph size".into()));
                    }
                    glyphs.insert(id as u32, glyph);
                }
                "kerning" => {
                    let (first, second) = (required("first")?, required("second")?);
                    if first >= 0 && second >= 0 {
                        kernings.insert((first as u32, second as u32), required("amount")?);
                    }
                }
                _ => {}
            }
        }
        let line_height = line_height.ok_or(FontError::Missing("a common entry"))?;
        if line_height <= 0.0 {
            return Err(FontError::Line {
                line: 0,
                message: "lineHeight must be positive".into(),
            });
        }
        if glyphs.is_empty() {
            return Err(FontError::Missing("char entries"));
        }
        Ok(Self {
            line_height,
            base: base.unwrap_or(line_height),
            scale_w: atlas.0,
            scale_h: atlas.1,
            padding,
            spacing,
            page: page.ok_or(FontError::Missing("a page entry"))?,
            glyphs,
            kernings,
        })
    }

    pub fn glyph(&self, character: char) -> Option<&Glyph> {
        self.glyphs.get(&(character as u32))
    }

    pub fn kerning(&self, first: u32, second: u32) -> i32 {
        self.kernings.get(&(first, second)).copied().unwrap_or(0)
    }

    /// The id and glyph drawn for `character`: its own, else `?`, else nothing.
    fn resolve(&self, character: char) -> Option<(u32, &Glyph)> {
        let own = character as u32;
        if let Some(glyph) = self.glyphs.get(&own) {
            return Some((own, glyph));
        }
        let fallback = u32::from(b'?');
        self.glyphs.get(&fallback).map(|g| (fallback, g))
    }

    /// Lay `text` out in font pixels. A character with no glyph (and no `?`) is skipped and
    /// breaks kerning; kerning pairs use the glyph actually drawn.
    pub fn layout(&self, text: &str) -> Layout {
        let mut glyphs = Vec::new();
        let mut line_widths = Vec::new();
        let mut pen = 0.0_f32;
        let mut line = 0;
        let mut previous: Option<u32> = None;
        for ch in text.chars() {
            if ch == '\n' {
                line_widths.push(pen);
                pen = 0.0;
                line += 1;
                previous = None;
                continue;
            }
            let Some((id, glyph)) = self.resolve(ch) else {
                previous = None;
                continue;
            };
            if let Some(prev) = previous {
                pen += self.kerning(prev, id) as f32;
            }
            glyphs.push(Placed {
                glyph: *glyph,
                x: pen,
                line,
            });
            pen += glyph.x_advance as f32;
            previous = Some(id);
        }
        line_widths.push(pen);
        Layout {
            glyphs,
            line_widths,
        }
    }

    /// Break `text` into lines no wider than `max_width` font pixels at word gaps. Explicit
    /// newlines stay; a word wider than the limit gets a line of its own.
    pub fn wrap(&self, text: &str, max_width: f32) -> String {
        let mut out = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty() && self.layout(&candidate).width() > max_width {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            out.push(line);
        }
        out.join("\n")
    }
}

fn parse_line(line: &str) -> (String, HashMap<String, String>) {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            c => token.push(c),
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
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

fn get_list<const N: usize>(attrs: &HashMap<String, String>, key: &str) -> Option<[i32; N]> {
    let parts: Vec<i32> = attrs
        .get(key)?
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = r#"
info face="Test Font" size=8 padding=1,2,3,4 spacing=1,1
common lineHeight=10 base=8 scaleW=16 scaleH=16 pages=1 packed=0
page id=0 file="font atlas.png"
chars count=4
char id=63 x=0 y=8 width=4 height=7 xoffset=0 yoffset=1 xadvance=5 page=0 chnl=0
char id=65 x=0 y=0 width=5 height=7 xoffset=0 yoffset=1 xadvance=6 page=0 chnl=0
char id=86 x=5 y=0 width=5 height=7 xoffset=0 yoffset=1 xadvance=6 page=0 chnl=0
char id=32 x=0 y=0 width=0 height=0 xoffset=0 yoffset=0 xadvance=3 page=0 chnl=0
kernings count=2
kerning first=65 second=86 amount=-1
kerning first=63 second=65 amount=-2
"#;

    fn font() -> FontData {
        FontData::parse(FONT).unwrap()
    }

    #[test]
    fn parses_metrics_page_padding_glyphs_and_kerning() {
        let font = font();
        assert_eq!((font.line_height, font.base), (10.0, 8.0));
        assert_eq!(font.page, "font atlas.png");
        assert_eq!((font.padding, font.spacing), ([1, 2, 3, 4], [1, 1]));
        assert_eq!(font.glyph('A').unwrap().x_advance, 6);
        assert_eq!(font.kerning(65, 86), -1);
        assert_eq!(font.kerning(86, 65), 0);
    }

    #[test]
    fn bad_input_is_an_error_not_a_default() {
        assert_eq!(
            FontData::parse("BMF\u{3}rest"),
            Err(FontError::Unsupported("the binary format"))
        );
        assert!(matches!(
            FontData::parse(&FONT.replace("pages=1", "pages=2")),
            Err(FontError::Unsupported(_))
        ));
        assert!(matches!(
            FontData::parse(
                &FONT.replace("xadvance=6 page=0 chnl=0\nchar id=86", "page=0\nchar id=86")
            ),
            Err(FontError::Line { .. })
        ));
        assert_eq!(
            FontData::parse("common lineHeight=5 base=4\npage id=0 file=\"a.png\""),
            Err(FontError::Missing("char entries"))
        );
    }

    #[test]
    fn layout_applies_kerning_and_starts_a_line_at_zero() {
        let layout = font().layout("AV\nV");
        let xs: Vec<f32> = layout.glyphs.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![0.0, 5.0, 0.0]); // A advances 6, kerning -1 before V
        assert_eq!(layout.line_widths, vec![11.0, 6.0]);
        assert_eq!(layout.width(), 11.0);
    }

    #[test]
    fn a_missing_glyph_draws_as_question_mark_and_kerns_as_one() {
        let layout = font().layout("Z");
        assert_eq!(layout.glyphs[0].glyph, *font().glyph('?').unwrap());
        // Kerning pair (63, 65) applies after the fallback.
        let layout = font().layout("ZA");
        assert_eq!(layout.glyphs[1].x, 5.0 - 2.0);
    }

    #[test]
    fn without_a_question_mark_a_missing_glyph_is_skipped() {
        let no_fallback = FONT.replace("char id=63", "char id=64");
        let layout = FontData::parse(&no_fallback).unwrap().layout("AZA");
        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[1].x, 6.0); // no kerning across the gap, no advance for Z
    }

    #[test]
    fn alignment_shifts_a_line_from_its_anchor() {
        assert_eq!(Align::Left.shift(10.0), 0.0);
        assert_eq!(Align::Center.shift(10.0), -5.0);
        assert_eq!(Align::Right.shift(10.0), -10.0);
    }

    #[test]
    fn wrap_breaks_at_word_gaps_and_keeps_explicit_newlines() {
        let font = font();
        // "A A" is 6 + 3 + 6 = 15 wide.
        assert_eq!(font.wrap("A A", 15.0), "A A");
        assert_eq!(font.wrap("A A", 14.0), "A\nA");
        assert_eq!(font.wrap("A\nA A", 100.0), "A\nA A");
        // A single word wider than the limit keeps its own line.
        assert_eq!(font.wrap("AAAA A", 5.0), "AAAA\nA");
    }

    #[test]
    fn the_atlas_size_is_read() {
        let font = font();
        assert_eq!((font.scale_w, font.scale_h), (16, 16));
    }

    #[test]
    fn the_original_outline_is_four_sides_and_the_lower_right() {
        assert_eq!(OutlineShape::Full.offsets().len(), 8);
        assert_eq!(OutlineShape::Cross.offsets().len(), 4);
        let original = OutlineShape::CrossAndLowerRight.offsets();
        assert_eq!(original.len(), 5);
        assert!(original.contains(&(1.0, 1.0)) && !original.contains(&(-1.0, -1.0)));
    }

    #[test]
    fn colour_sets_are_told_apart_by_their_average_and_lightness() {
        // Two sets of one row of two pixels: red then a dim grey with a hole.
        let rgba = [
            255, 0, 0, 255, 255, 0, 0, 255, // set 0
            100, 100, 100, 255, 0, 0, 0, 0, // set 1
        ];
        let averages = set_averages(&rgba, 2, 2, 2);
        assert_eq!(averages[0], [1.0, 0.0, 0.0]);
        assert!((averages[1][0] - 100.0 / 255.0).abs() < 1e-6);
        assert_eq!(nearest_set(&averages, [0.9, 0.1, 0.0]), 0);
        assert_eq!(nearest_set(&averages, [0.4, 0.4, 0.4]), 1);
        assert_eq!(lightest_set(&averages), 1);
        // No drawn pixel is black, never a division by zero.
        assert_eq!(set_averages(&[0; 8], 2, 1, 1), vec![[0.0; 3]]);
    }
}
