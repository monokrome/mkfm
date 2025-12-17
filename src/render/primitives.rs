//! Primitive drawing helpers

use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer, VAlign};

use super::{RenderColors, RenderLayout};

/// Draw a 1px border around a rectangle
pub fn draw_border(canvas: &mut Canvas, x: i32, y: i32, w: u32, h: u32, color: Color) {
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
    canvas.fill_rect(x, y, w, 1.0, color);
    canvas.fill_rect(x, y + h - 1.0, w, 1.0, color);
    canvas.fill_rect(x, y, 1.0, h, color);
    canvas.fill_rect(x + w - 1.0, y, 1.0, h, color);
}

/// Draw a filled row background
pub fn draw_row_bg(canvas: &mut Canvas, x: i32, y: i32, w: u32, h: i32, color: Color) {
    canvas.fill_rect(x as f32, y as f32, w as f32, h as f32, color);
}

/// Draw a header bar with background and text
#[allow(clippy::too_many_arguments)]
pub fn draw_header(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    x: i32,
    y: i32,
    w: u32,
    text: &str,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    canvas.fill_rect(
        x as f32,
        y as f32,
        w as f32,
        layout.header_height as f32,
        colors.header_bg,
    );
    let rect = Rect::new(
        x + layout.padding,
        y,
        w.saturating_sub(layout.padding as u32 * 2),
        layout.header_height as u32,
    );
    tr.draw_text_in_rect(
        canvas,
        text,
        rect,
        layout.font_size,
        colors.fg,
        HAlign::Left,
        VAlign::Center,
    );
}

/// Draw text in a rect with specified alignment
pub fn draw_text(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    text: &str,
    rect: Rect,
    font_size: f32,
    color: TextColor,
    halign: HAlign,
) {
    tr.draw_text_in_rect(canvas, text, rect, font_size, color, halign, VAlign::Center);
}

/// Determine and draw the appropriate row background based on state
#[allow(clippy::too_many_arguments)]
pub fn draw_list_row_bg(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: u32,
    h: i32,
    is_cursor: bool,
    is_selected: bool,
    is_search_match: bool,
    colors: &RenderColors,
) {
    let bg = if is_cursor {
        Some(colors.cursor_bg)
    } else if is_search_match {
        Some(colors.search_highlight_bg)
    } else if is_selected {
        Some(colors.selected_bg)
    } else {
        None
    };
    if let Some(color) = bg {
        draw_row_bg(canvas, x, y, w, h, color);
    }
}
