//! Preview rendering functions

use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer, VAlign};

use super::{MediaMetadata, MediaType, PreviewContent};

/// Render preview content to canvas
pub fn render_preview(
    canvas: &mut Canvas,
    text_renderer: &mut TextRenderer,
    content: &PreviewContent,
) {
    canvas.clear(Color::from_rgba8(0, 0, 0, 0));

    match content {
        PreviewContent::Image {
            data,
            width: img_w,
            height: img_h,
        } => {
            canvas.draw_rgba(0, 0, *img_w, *img_h, data);
        }
        PreviewContent::Text(lines) => {
            render_text_preview(canvas, text_renderer, lines);
        }
        PreviewContent::Media {
            media_type,
            metadata,
            thumbnail,
            thumb_width,
            thumb_height,
        } => {
            render_media_preview(
                canvas,
                text_renderer,
                *media_type,
                metadata,
                thumbnail.as_deref(),
                *thumb_width,
                *thumb_height,
            );
        }
        PreviewContent::Unsupported(filename) => {
            render_message(
                canvas,
                text_renderer,
                &format!("No preview for: {}", filename),
                false,
            );
        }
        PreviewContent::Error(msg) => {
            render_message(canvas, text_renderer, msg, true);
        }
    }
}

fn render_message(canvas: &mut Canvas, tr: &mut TextRenderer, msg: &str, is_error: bool) {
    let full_rect = Rect::new(0, 0, canvas.width(), canvas.height());
    let color = if is_error {
        TextColor::rgb(200, 100, 100)
    } else {
        TextColor::rgb(150, 150, 150)
    };
    tr.draw_text_in_rect(
        canvas,
        msg,
        full_rect,
        14.0,
        color,
        HAlign::Center,
        VAlign::Center,
    );
}

fn render_text_preview(canvas: &mut Canvas, tr: &mut TextRenderer, lines: &[String]) {
    let width = canvas.width();
    let height = canvas.height();
    let font_size = 12.0;
    let line_height = 16;

    for (i, line) in lines.iter().enumerate() {
        let y = i as i32 * line_height;
        if y + line_height > height as i32 {
            break;
        }
        let line_rect = Rect::new(0, y, width, line_height as u32);
        let display_line = if line.len() > 80 {
            &line[..80]
        } else {
            line.as_str()
        };
        tr.draw_text_in_rect(
            canvas,
            display_line,
            line_rect,
            font_size,
            TextColor::rgb(200, 200, 200),
            HAlign::Left,
            VAlign::Top,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_media_preview(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    media_type: MediaType,
    metadata: &MediaMetadata,
    thumbnail: Option<&[u8]>,
    thumb_width: u32,
    thumb_height: u32,
) {
    let width = canvas.width();
    let font_size = 12.0;
    let line_height = 16i32;
    let padding = 8i32;

    let mut y = padding;

    // Draw thumbnail if available (video)
    if let Some(data) = thumbnail
        && thumb_width > 0
        && thumb_height > 0
    {
        canvas.draw_rgba(padding, y, thumb_width, thumb_height, data);
        y += thumb_height as i32 + padding;
    }

    render_media_type_label(
        canvas,
        tr,
        media_type,
        padding,
        &mut y,
        width,
        font_size,
        line_height,
    );
    render_media_tags(
        canvas,
        tr,
        metadata,
        padding,
        &mut y,
        width,
        font_size,
        line_height,
    );
    render_media_info(
        canvas,
        tr,
        metadata,
        padding,
        &mut y,
        width,
        font_size,
        line_height,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_media_type_label(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    media_type: MediaType,
    padding: i32,
    y: &mut i32,
    width: u32,
    font_size: f32,
    line_height: i32,
) {
    let type_label = match media_type {
        MediaType::Audio => "♪ Audio",
        MediaType::Video => "▶ Video",
    };
    let label_rect = Rect::new(padding, *y, width - padding as u32 * 2, line_height as u32);
    tr.draw_text_in_rect(
        canvas,
        type_label,
        label_rect,
        font_size + 2.0,
        TextColor::rgb(100, 180, 255),
        HAlign::Left,
        VAlign::Top,
    );
    *y += line_height + 4;
}

#[allow(clippy::too_many_arguments)]
fn render_media_tags(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    metadata: &MediaMetadata,
    padding: i32,
    y: &mut i32,
    width: u32,
    font_size: f32,
    line_height: i32,
) {
    if let Some(ref title) = metadata.title {
        draw_line(canvas, tr, title, padding, y, width, font_size, line_height);
    }
    if let Some(ref artist) = metadata.artist {
        draw_line(
            canvas,
            tr,
            &format!("by {}", artist),
            padding,
            y,
            width,
            font_size,
            line_height,
        );
    }
    if let Some(ref album) = metadata.album {
        draw_line(
            canvas,
            tr,
            &format!("on {}", album),
            padding,
            y,
            width,
            font_size,
            line_height,
        );
    }
    *y += 4;
}

#[allow(clippy::too_many_arguments)]
fn render_media_info(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    metadata: &MediaMetadata,
    padding: i32,
    y: &mut i32,
    width: u32,
    font_size: f32,
    line_height: i32,
) {
    if let Some(duration) = metadata.format_duration() {
        draw_line(
            canvas,
            tr,
            &format!("Duration: {}", duration),
            padding,
            y,
            width,
            font_size,
            line_height,
        );
    }

    if let Some(ref codec) = metadata.codec {
        let mut info = codec.to_uppercase();
        if let Some(br) = metadata.bitrate {
            info.push_str(&format!(" @ {} kbps", br / 1000));
        }
        draw_line(canvas, tr, &info, padding, y, width, font_size, line_height);
    }

    if let (Some(w), Some(h)) = (metadata.width, metadata.height) {
        draw_line(
            canvas,
            tr,
            &format!("{}x{}", w, h),
            padding,
            y,
            width,
            font_size,
            line_height,
        );
    }

    if let Some(sr) = metadata.sample_rate {
        let mut audio_info = format!("{} Hz", sr);
        if let Some(ch) = metadata.channels {
            audio_info.push_str(&format!(", {} ch", ch));
        }
        draw_line(
            canvas,
            tr,
            &audio_info,
            padding,
            y,
            width,
            font_size,
            line_height,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    canvas: &mut Canvas,
    tr: &mut TextRenderer,
    text: &str,
    padding: i32,
    y: &mut i32,
    width: u32,
    font_size: f32,
    line_height: i32,
) {
    let rect = Rect::new(padding, *y, width - padding as u32 * 2, line_height as u32);
    tr.draw_text_in_rect(
        canvas,
        text,
        rect,
        font_size,
        TextColor::rgb(200, 200, 200),
        HAlign::Left,
        VAlign::Top,
    );
    *y += line_height;
}
