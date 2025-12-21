//! Preview content loading for different file types

use image::GenericImageView;

use super::{PreviewContent, is_image_file, is_media_file, is_svg_file, is_text_file};

/// Load preview content based on file type
pub fn load_preview_content(
    path: &std::path::Path,
    max_width: u32,
    max_height: u32,
) -> PreviewContent {
    if is_svg_file(path) {
        load_svg_preview(path, max_width, max_height)
    } else if is_image_file(path) {
        load_image_preview(path, max_width, max_height)
    } else if is_media_file(path) {
        load_media_preview(path, max_width, max_height)
    } else if is_text_file(path) {
        load_text_preview(path)
    } else {
        load_unsupported(path)
    }
}

fn load_svg_preview(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    match try_load_svg(path, max_width, max_height) {
        Ok(content) => content,
        Err(e) => PreviewContent::Error(format!("SVG load failed: {}", e)),
    }
}

fn try_load_svg(
    path: &std::path::Path,
    max_width: u32,
    max_height: u32,
) -> Result<PreviewContent, Box<dyn std::error::Error>> {
    let svg_data = std::fs::read(path)?;
    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())?;

    let svg_size = tree.size();
    let (svg_w, svg_h) = (svg_size.width(), svg_size.height());

    let scale = calculate_scale(svg_w, svg_h, max_width as f32, max_height as f32);
    let (target_w, target_h) = apply_scale(svg_w, svg_h, scale);

    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(target_w, target_h).ok_or("Failed to create pixmap")?;

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let data: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| [p.red(), p.green(), p.blue(), p.alpha()])
        .collect();

    Ok(PreviewContent::Image {
        data,
        width: target_w,
        height: target_h,
    })
}

fn load_image_preview(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    match image::open(path) {
        Ok(img) => process_image(img, max_width, max_height),
        Err(e) => PreviewContent::Error(format!("Load failed: {}", e)),
    }
}

fn process_image(img: image::DynamicImage, max_width: u32, max_height: u32) -> PreviewContent {
    let (img_w, img_h) = img.dimensions();

    if img_w == 0 || img_h == 0 {
        return PreviewContent::Error("Invalid image dimensions".to_string());
    }

    let scale = calculate_scale(
        img_w as f32,
        img_h as f32,
        max_width as f32,
        max_height as f32,
    );
    let target_w = ((img_w as f32 * scale) as u32).max(1);
    let target_h = ((img_h as f32 * scale) as u32).max(1);

    let rgba = if target_w < img_w || target_h < img_h {
        img.resize_exact(target_w, target_h, image::imageops::FilterType::Triangle)
            .to_rgba8()
    } else {
        img.to_rgba8()
    };

    let (final_w, final_h) = rgba.dimensions();
    PreviewContent::Image {
        data: rgba.into_raw(),
        width: final_w,
        height: final_h,
    }
}

fn load_media_preview(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    if crate::ffmpeg::is_available() {
        crate::ffmpeg::load_media_preview(path, max_width, max_height)
    } else {
        PreviewContent::Unsupported("Media (ffmpeg not found)".to_string())
    }
}

fn load_text_preview(path: &std::path::Path) -> PreviewContent {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<String> = content.lines().take(50).map(|s| s.to_string()).collect();
            PreviewContent::Text(lines)
        }
        Err(e) => PreviewContent::Error(format!("Read failed: {}", e)),
    }
}

fn load_unsupported(path: &std::path::Path) -> PreviewContent {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");
    PreviewContent::Unsupported(filename.to_string())
}

fn calculate_scale(src_w: f32, src_h: f32, max_w: f32, max_h: f32) -> f32 {
    let scale_x = max_w / src_w;
    let scale_y = max_h / src_h;
    scale_x.min(scale_y).min(1.0)
}

fn apply_scale(w: f32, h: f32, scale: f32) -> (u32, u32) {
    ((w * scale).round() as u32, (h * scale).round() as u32)
}
