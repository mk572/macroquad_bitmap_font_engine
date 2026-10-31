use macroquad::prelude::*;
use macroquad_bitmap_font_engine::BmFont;

#[macroquad::main("BMFont example")]
async fn main() {
    // Place your BMFont text export and atlas at assets/font.fnt and assets/font.png.
    let font = BmFont::load("assets/font.fnt")
        .await
        .expect("could not load assets/font.fnt");

    loop {
        clear_background(Color::from_rgba(24, 28, 36, 255));
        font.draw_text(
            "Hello, bitmap font!\nAV kerning and line breaks",
            24.0,
            52.0,
            2.0,
            WHITE,
        );
        let size = font.measure_text("Hello, bitmap font!", 2.0);
        draw_text(
            &format!("Measured advance: {:.0} x {:.0}", size.width, size.height),
            24.0,
            120.0,
            18.0,
            LIGHTGRAY,
        );
        next_frame().await;
    }
}
