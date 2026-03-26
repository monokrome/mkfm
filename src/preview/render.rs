//! Preview rendering — renders preview content into a region using mkui Renderer

use mkui::layout::{ObjectFit, Rect};
use mkui::render::{ImageParams, Renderer};
use mkui::style::Style;
use mkui::theme::Color;

use super::PreviewContent;

/// Render preview content into the given bounds
pub fn render_preview(
    renderer: &mut dyn Renderer,
    content: &PreviewContent,
    bounds: Rect,
    fg: Color,
    bg: Color,
) {
    // Background
    let _ = renderer.fill_rect(bounds, bg);

    match content {
        PreviewContent::Image {
            data,
            width,
            height,
        } => render_image_preview(renderer, data, *width, *height, bounds),
        PreviewContent::Text(lines) => render_text_preview(renderer, lines, bounds, fg),
        PreviewContent::Media {
            media_type,
            metadata,
            thumbnail,
            thumb_width,
            thumb_height,
        } => render_media_preview(
            renderer,
            *media_type,
            metadata,
            thumbnail.as_deref(),
            *thumb_width,
            *thumb_height,
            bounds,
            fg,
        ),
        PreviewContent::Unsupported(msg) => render_message(renderer, msg, bounds, fg),
        PreviewContent::Error(msg) => {
            render_message(renderer, msg, bounds, Color::rgb(255, 100, 100))
        }
    }
}

fn render_image_preview(
    renderer: &mut dyn Renderer,
    data: &[u8],
    width: u32,
    height: u32,
    bounds: Rect,
) {
    let dst = ObjectFit::Contain.fit_with_aspect(width, height, bounds, renderer.cell_aspect());

    // Preview images are loaded as RGBA
    let _ = renderer.render_image_rgba(&ImageParams {
        data,
        width,
        height,
        col: dst.x,
        row: dst.y,
        width_cells: Some(dst.width),
        height_cells: Some(dst.height),
    });
}

fn render_text_preview(
    renderer: &mut dyn Renderer,
    lines: &[String],
    bounds: Rect,
    fg: Color,
) {
    let style = Style::new().fg(fg);
    let max_lines = bounds.height as usize;
    let max_width = bounds.width as usize;

    for (i, line) in lines.iter().take(max_lines).enumerate() {
        let row = bounds.y + i as u16;
        let _ = renderer.move_cursor(bounds.x, row);

        let display = if line.len() > max_width {
            &line[..max_width]
        } else {
            line.as_str()
        };
        let _ = renderer.write_styled(display, &style);
    }
}

fn render_media_preview(
    renderer: &mut dyn Renderer,
    media_type: super::MediaType,
    metadata: &super::MediaMetadata,
    thumbnail: Option<&[u8]>,
    thumb_width: u32,
    thumb_height: u32,
    bounds: Rect,
    fg: Color,
) {
    let _style = Style::new().fg(fg);
    let dim_style = Style::new().fg(fg).dim(true);
    let mut row = bounds.y;

    // Thumbnail
    if let Some(data) = thumbnail {
        if thumb_width > 0 && thumb_height > 0 {
            let thumb_rows = bounds.height / 2;
            let thumb_bounds = Rect::new(bounds.x, row, bounds.width, thumb_rows);
            let dst = ObjectFit::Contain.fit(thumb_width, thumb_height, thumb_bounds);

            let _ = renderer.render_image_rgba(&ImageParams {
                data,
                width: thumb_width,
                height: thumb_height,
                col: dst.x,
                row: dst.y,
                width_cells: Some(dst.width),
                height_cells: Some(dst.height),
            });
            row += thumb_rows;
        }
    }

    // Media type
    let type_str = match media_type {
        super::MediaType::Audio => "Audio",
        super::MediaType::Video => "Video",
    };
    if row < bounds.y + bounds.height {
        let _ = renderer.move_cursor(bounds.x, row);
        let _ = renderer.write_styled(type_str, &Style::new().fg(fg).bold(true));
        row += 1;
    }

    // Metadata lines
    let mut info_lines = Vec::new();

    if let Some(title) = &metadata.title {
        info_lines.push(format!("  {title}"));
    }
    if let Some(artist) = &metadata.artist {
        info_lines.push(format!("  {artist}"));
    }
    if let Some(album) = &metadata.album {
        info_lines.push(format!("  {album}"));
    }
    if let Some(dur) = metadata.format_duration() {
        info_lines.push(format!("  Duration: {dur}"));
    }
    if let Some(codec) = &metadata.codec {
        info_lines.push(format!("  Codec: {codec}"));
    }
    if let (Some(w), Some(h)) = (metadata.width, metadata.height) {
        info_lines.push(format!("  {w}x{h}"));
    }

    for line in &info_lines {
        if row >= bounds.y + bounds.height {
            break;
        }
        let _ = renderer.move_cursor(bounds.x, row);
        let max_w = bounds.width as usize;
        let display = if line.len() > max_w {
            &line[..max_w]
        } else {
            line.as_str()
        };
        let _ = renderer.write_styled(display, &dim_style);
        row += 1;
    }
}

fn render_message(renderer: &mut dyn Renderer, msg: &str, bounds: Rect, color: Color) {
    let _ = renderer.move_cursor(bounds.x + 1, bounds.y + 1);
    let _ = renderer.write_styled(msg, &Style::new().fg(color));
}
