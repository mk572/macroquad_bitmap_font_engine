//! Tiny desktop BMFont previewer. Run with: cargo run --example viewer
//! Uses a native file picker, so it can open .fnt files from any local folder.
use macroquad::prelude::*;
use macroquad_bitmap_font_engine::BmFont;

fn inside(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

#[macroquad::main("Macroquad BMFont Viewer")]
async fn main() {
    let mut font: Option<BmFont> = None;
    let mut font_path = String::from("Choose a BMFont .fnt file");
    let mut error = String::new();
    let mut zoom = 3.0_f32;
    let mut sample =
        String::from("The quick brown fox jumps over the lazy dog.\n0123456789  AVATAR  To Wa Yo");
    let mut sample_input = false;

    loop {
        let sw = screen_width();
        let sh = screen_height();
        clear_background(Color::from_rgba(25, 28, 35, 255));

        draw_text("BMFont Viewer", 20.0, 30.0, 26.0, WHITE);

        let open_rect = (20.0, 48.0, 130.0, 34.0);
        draw_rectangle(
            open_rect.0,
            open_rect.1,
            open_rect.2,
            open_rect.3,
            Color::from_rgba(65, 95, 145, 255),
        );
        draw_rectangle_lines(
            open_rect.0,
            open_rect.1,
            open_rect.2,
            open_rect.3,
            1.0,
            SKYBLUE,
        );
        draw_text(
            "Open .fnt...",
            open_rect.0 + 12.0,
            open_rect.1 + 23.0,
            18.0,
            WHITE,
        );
        draw_text(&font_path, 165.0, 70.0, 16.0, LIGHTGRAY);

        let (mx, my) = mouse_position();
        if is_mouse_button_pressed(MouseButton::Left)
            && inside(mx, my, open_rect.0, open_rect.1, open_rect.2, open_rect.3)
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("AngelCode BMFont text", &["fnt"])
                .set_title("Open BMFont text file")
                .pick_file()
            {
                let chosen = path.to_string_lossy().to_string();
                match BmFont::load(&chosen).await {
                    Ok(loaded) => {
                        font = Some(loaded);
                        font_path = chosen;
                        error.clear();
                    }
                    Err(e) => {
                        error = e;
                        font_path = chosen;
                        font = None;
                    }
                }
            }
        }

        draw_text("Zoom", 20.0, 112.0, 18.0, WHITE);
        let slider_x = 85.0;
        let slider_y = 103.0;
        let slider_w = 250.0;
        draw_rectangle(slider_x, slider_y, slider_w, 8.0, DARKGRAY);
        let knob_x = slider_x + ((zoom - 1.0) / 7.0) * slider_w;
        draw_circle(knob_x, slider_y + 4.0, 8.0, SKYBLUE);
        draw_text(
            format!("{zoom:.1}x"),
            slider_x + slider_w + 18.0,
            112.0,
            18.0,
            WHITE,
        );
        if is_mouse_button_down(MouseButton::Left)
            && (inside(
                mx,
                my,
                slider_x - 8.0,
                slider_y - 10.0,
                slider_w + 16.0,
                28.0,
            ))
        {
            zoom = (1.0 + ((mx - slider_x) / slider_w).clamp(0.0, 1.0) * 7.0).clamp(1.0, 8.0);
        }

        draw_text(
            "Preview text (click the input box and type):",
            20.0,
            153.0,
            17.0,
            LIGHTGRAY,
        );
        let input_rect = (20.0, 164.0, (sw - 40.0).max(120.0), 34.0);
        draw_rectangle(
            input_rect.0,
            input_rect.1,
            input_rect.2,
            input_rect.3,
            Color::from_rgba(40, 44, 53, 255),
        );
        draw_rectangle_lines(
            input_rect.0,
            input_rect.1,
            input_rect.2,
            input_rect.3,
            1.0,
            GRAY,
        );
        draw_text(
            &sample,
            input_rect.0 + 8.0,
            input_rect.1 + 22.0,
            16.0,
            WHITE,
        );
        if is_mouse_button_pressed(MouseButton::Left) {
            sample_input = inside(
                mx,
                my,
                input_rect.0,
                input_rect.1,
                input_rect.2,
                input_rect.3,
            );
        }
        if sample_input {
            while let Some(c) = get_char_pressed() {
                if !c.is_control() && sample.len() < 240 {
                    sample.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                sample.pop();
            }
            if is_key_pressed(KeyCode::Enter) {
                sample.push('\n');
            }
        }

        if !error.is_empty() {
            draw_text(
                format!("Could not load font: {error}"),
                20.0,
                224.0,
                17.0,
                RED,
            );
        } else if let Some(font) = &font {
            draw_text("Rendered preview", 20.0, 226.0, 18.0, WHITE);
            let preview_y = 250.0 + font.base * zoom;
            font.draw_text(
                &sample,
                24.0,
                preview_y,
                zoom,
                Color::from_rgba(255, 245, 210, 255),
            );

            let measured = font.measure_text(&sample, zoom);
            draw_text(
                format!(
                    "Advance size: {:.0} × {:.0} px   Line height: {:.0} px",
                    measured.width,
                    measured.height,
                    font.line_height * zoom
                ),
                20.0,
                (sh - 22.0).max(240.0),
                16.0,
                LIGHTGRAY,
            );
        } else {
            draw_text(
                "Open a text-format BMFont .fnt file to preview it.",
                20.0,
                225.0,
                18.0,
                LIGHTGRAY,
            );
            draw_text(
                "The atlas PNG must be in the location specified by the .fnt file.",
                20.0,
                250.0,
                16.0,
                GRAY,
            );
        }

        draw_text("Esc: quit", (sw - 100.0).max(20.0), 30.0, 15.0, GRAY);
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        next_frame().await;
    }
}
