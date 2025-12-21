//! Feature panel rendering

use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer, VAlign};

use crate::config::Theme;
use crate::features::{Feature, FeatureList, FeatureListPane};

use super::primitives::{draw_border, draw_header, draw_row_bg, draw_text};
use super::{RenderColors, RenderLayout};

/// Render the feature list overlay panel
#[allow(clippy::too_many_arguments)]
pub fn render_feature_panel(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    features: &FeatureList,
    pane: &FeatureListPane,
    width: u32,
    height: u32,
    is_focused: bool,
    theme: &Theme,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let panel_w = (width as f32 * 0.6).min(500.0) as u32;
    let panel_h =
        calculate_panel_height(features.features.len(), pane.showing_detail, height, layout);
    let panel_x = (width - panel_w) as i32 / 2;
    let panel_y = (height - panel_h) as i32 / 2;

    render_dim_overlay(canvas, width, height);
    render_panel_background(canvas, theme, panel_x, panel_y, panel_w, panel_h);

    let border = if is_focused {
        colors.border_focused
    } else {
        colors.border
    };
    draw_border(canvas, panel_x, panel_y, panel_w, panel_h, border);

    render_panel_header(
        canvas, tr, features, panel_x, panel_y, panel_w, colors, layout,
    );

    let content_y = panel_y + 1 + layout.header_height;
    let content_h = panel_h as i32 - layout.header_height - 2;
    let visible = (content_h / layout.line_height).max(0) as usize;

    render_feature_list(
        canvas, tr, features, pane, panel_x, content_y, panel_w, visible, colors, layout,
    );

    if pane.showing_detail {
        render_detail_section(
            canvas, tr, features, pane, panel_x, content_y, panel_w, panel_h, panel_y, visible,
            colors, layout,
        );
    } else {
        render_hint_text(canvas, tr, panel_x, panel_y, panel_w, panel_h, layout);
    }
}

fn calculate_panel_height(count: usize, detail: bool, max: u32, layout: &RenderLayout) -> u32 {
    let base = if detail { 100 } else { 60 };
    (count as u32 * layout.line_height as u32 + base).min(max - 100)
}

fn render_dim_overlay(canvas: &mut Canvas, width: u32, height: u32) {
    canvas.fill_rect(
        0.0,
        0.0,
        width as f32,
        height as f32,
        Color::from_rgba8(0, 0, 0, 180),
    );
}

fn render_panel_background(canvas: &mut Canvas, theme: &Theme, x: i32, y: i32, w: u32, h: u32) {
    let bg = Color::from_rgba8(
        theme.background.r,
        theme.background.g,
        theme.background.b,
        250,
    );
    canvas.fill_rect(x as f32, y as f32, w as f32, h as f32, bg);
}

#[allow(clippy::too_many_arguments)]
fn render_panel_header(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    features: &FeatureList,
    x: i32,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    let header_text = format!(
        "Features ({} available, {} unavailable) - F12 to close",
        features.available_count(),
        features.unavailable_count()
    );
    draw_header(
        canvas,
        tr,
        x + 1,
        y + 1,
        w - 2,
        &header_text,
        colors,
        layout,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_feature_list(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    features: &FeatureList,
    pane: &FeatureListPane,
    panel_x: i32,
    content_y: i32,
    panel_w: u32,
    visible: usize,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    for (i, feature) in features.features.iter().enumerate().take(visible) {
        let row_y = content_y + (i as i32 * layout.line_height);
        render_feature_row(
            canvas,
            tr,
            feature,
            i == pane.cursor,
            panel_x + 1,
            row_y,
            panel_w - 2,
            colors,
            layout,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_feature_row(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    feature: &Feature,
    is_cursor: bool,
    x: i32,
    y: i32,
    w: u32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    if is_cursor {
        draw_row_bg(canvas, x, y, w, layout.line_height, colors.cursor_bg);
    }

    let (icon, icon_color) = if feature.available {
        ("\u{f00c}", TextColor::rgb(100, 200, 100))
    } else {
        ("\u{f00d}", TextColor::rgb(200, 100, 100))
    };

    let icon_rect = Rect::new(x + layout.padding, y, 20, layout.line_height as u32);
    draw_text(
        canvas,
        tr,
        icon,
        icon_rect,
        layout.font_size,
        icon_color,
        HAlign::Left,
    );

    let name_rect = Rect::new(
        x + layout.padding + 24,
        y,
        w - layout.padding as u32 * 2 - 24,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        feature.name,
        name_rect,
        layout.font_size,
        colors.fg,
        HAlign::Left,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_detail_section(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    features: &FeatureList,
    pane: &FeatureListPane,
    panel_x: i32,
    content_y: i32,
    panel_w: u32,
    panel_h: u32,
    panel_y: i32,
    visible: usize,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    if let Some(feature) = features.features.get(pane.cursor) {
        let detail_y = content_y + visible as i32 * layout.line_height + 8;
        let detail_h = panel_h as i32 - (visible as i32 * layout.line_height + 8 - panel_y);
        render_feature_detail(
            canvas, tr, feature, panel_x, detail_y, panel_w, detail_h, colors, layout,
        );
    }
}

fn render_hint_text(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    panel_x: i32,
    panel_y: i32,
    panel_w: u32,
    panel_h: u32,
    layout: &RenderLayout,
) {
    let hint_y = panel_y + panel_h as i32 - layout.line_height - 4;
    let hint_rect = Rect::new(
        panel_x + layout.padding,
        hint_y,
        panel_w - layout.padding as u32 * 2,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        "Press Enter for details, Escape to close",
        hint_rect,
        layout.font_size - 2.0,
        TextColor::rgb(128, 128, 128),
        HAlign::Center,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_feature_detail(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    feature: &Feature,
    x: i32,
    y: i32,
    w: u32,
    h: i32,
    colors: &RenderColors,
    layout: &RenderLayout,
) {
    if h <= 0 {
        return;
    }

    canvas.fill_rect(
        (x + layout.padding) as f32,
        (y - 4) as f32,
        (w as i32 - layout.padding * 2) as f32,
        1.0,
        colors.border,
    );

    let desc_rect = Rect::new(
        x + layout.padding,
        y,
        w - layout.padding as u32 * 2,
        layout.line_height as u32,
    );
    draw_text(
        canvas,
        tr,
        feature.description,
        desc_rect,
        layout.font_size - 1.0,
        colors.fg,
        HAlign::Left,
    );

    if let Some(ref reason) = feature.reason {
        let reason_rect = Rect::new(
            x + layout.padding,
            y + layout.line_height,
            w - layout.padding as u32 * 2,
            (h - layout.line_height).max(layout.line_height) as u32,
        );
        tr.draw_text_in_rect(
            canvas,
            reason,
            reason_rect,
            layout.font_size - 2.0,
            TextColor::rgb(200, 150, 100),
            HAlign::Left,
            VAlign::Top,
        );
    }
}
