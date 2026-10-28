# macroquad_bitmap_font_engine

A deliberately small AngelCode BMFont text-format renderer for Macroquad. It is intended for classic bitmap/pixel fonts and keeps the runtime API small.

## Features

- Loads the text .fnt format and its single PNG atlas page.
- Uses glyph rectangles, offsets, advances, baseline and pair kerning.
- draw_text supports a uniform scale, tint and newline-separated lines.
- measure_text returns advance width and line-box height.
- Sets the atlas to nearest-neighbour filtering for crisp pixel art.
- No extra runtime dependencies beyond Macroquad.

Not supported: binary .fnt, multiple atlas pages, packed/channel-specific glyphs, Unicode shaping, bidirectional layout or rich text.

## Quick start

Export a **text** BMFont from your font tool and keep its atlas PNG next to the .fnt file. For example:

    assets/
      font.fnt
      font.png

The page's file= value in the .fnt file must match the atlas filename. The example expects these paths; it does not include a font asset, so supply one you are licensed to use.

Add the crate as a path dependency (or publish/version it first):

    [dependencies]
    macroquad = "0.4"
    macroquad_bitmap_font_engine = { path = "../macroquad_bitmap_font_engine" }

Then:

    use macroquad::prelude::*;
    use macroquad_bitmap_font_engine::BmFont;

    #[macroquad::main("Bitmap font")]
    async fn main() {
        let font = BmFont::load("assets/font.fnt").await.unwrap();
        loop {
            clear_background(BLACK);
            font.draw_text("Hello!", 20.0, 50.0, 2.0, WHITE);
            let size = font.measure_text("Hello!", 2.0);
            next_frame().await;
        }
    }

The y coordinate passed to draw_text is the **baseline**, not the top of the glyph. Scale is relative to the BMFont's original pixel size. Unknown characters use ? when that glyph exists; otherwise they are skipped.

If you already loaded the atlas yourself, use BmFont::from_fnt_str(&fnt_text, texture). The atlas is configured to FilterMode::Nearest.

## Reproducible asset workflow

1. Keep the source font file and the exact BMFont exporter/version in your project's asset pipeline documentation.
2. Export text-format BMFont metadata and its PNG atlas with fixed settings (size, padding, spacing, packing, charset and output filenames).
3. Commit the generated .fnt and PNG together, or regenerate them in a pinned tool/container step.
4. Review generated asset diffs whenever the source font or export settings change. Do not silently substitute a system font.
5. Record the font's origin, version and license alongside the assets. Check whether the font license permits embedding and redistribution; a permissive engine license does not change the font's license.

The engine deliberately does not bundle a sample font, avoiding unclear font redistribution rights.

## Tests and example

Run parser unit tests with:

    cargo test

Run the example after adding your own font files:

    cargo run --example basic

## License

The engine is MIT licensed. Font files and atlases are separate assets and retain their own licenses.
