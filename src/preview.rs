//! Preview content loading and rendering

use std::path::PathBuf;

use image::GenericImageView;
use mkframe::{Canvas, Color, HAlign, Rect, TextColor, TextRenderer, VAlign};

/// Content types for file preview
pub enum PreviewContent {
    Image {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    Text(Vec<String>),
    Unsupported(String),
    Error(String),
}

impl PreviewContent {
    pub fn dimensions(&self, max_width: u32, max_height: u32) -> (u32, u32) {
        match self {
            PreviewContent::Image { width, height, .. } => (*width, *height),
            PreviewContent::Text(lines) => {
                let line_height = 16u32;
                let char_width = 8u32;
                let max_line_len = lines.iter().map(|l| l.len().min(80)).max().unwrap_or(20) as u32;
                let height = (lines.len() as u32 * line_height).min(max_height);
                let width = (max_line_len * char_width).min(max_width);
                (width.max(100), height.max(50))
            }
            PreviewContent::Unsupported(_) | PreviewContent::Error(_) => (200, 50),
        }
    }
}

/// Cache for loaded preview content
pub struct PreviewCache {
    path: Option<PathBuf>,
    content: Option<PreviewContent>,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self {
            path: None,
            content: None,
        }
    }

    pub fn get_or_load(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> &PreviewContent {
        if self.path.as_deref() != Some(path) {
            self.path = Some(path.to_path_buf());
            self.content = Some(self.load_content(path, max_width, max_height));
        }
        self.content.as_ref().unwrap()
    }

    fn load_content(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> PreviewContent {
        let path_owned = path.to_path_buf();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            load_preview_content(&path_owned, max_width, max_height)
        }))
        .unwrap_or_else(|_| PreviewContent::Error("Preview crashed".to_string()))
    }

    pub fn invalidate(&mut self) {
        self.path = None;
        self.content = None;
    }
}

/// Load SVG file as preview content
fn load_svg_preview(
    path: &std::path::Path,
    max_width: u32,
    max_height: u32,
) -> Result<PreviewContent, Box<dyn std::error::Error>> {
    let svg_data = std::fs::read(path)?;
    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())?;

    let svg_size = tree.size();
    let svg_w = svg_size.width();
    let svg_h = svg_size.height();

    let scale_x = max_width as f32 / svg_w;
    let scale_y = max_height as f32 / svg_h;
    let scale = scale_x.min(scale_y).min(1.0);

    let target_w = (svg_w * scale).round() as u32;
    let target_h = (svg_h * scale).round() as u32;

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

/// Load preview content based on file type
pub fn load_preview_content(
    path: &std::path::Path,
    max_width: u32,
    max_height: u32,
) -> PreviewContent {
    if is_svg_file(path) {
        match load_svg_preview(path, max_width, max_height) {
            Ok(content) => content,
            Err(e) => PreviewContent::Error(format!("SVG load failed: {}", e)),
        }
    } else if is_image_file(path) {
        load_image_preview(path, max_width, max_height)
    } else if is_text_file(path) {
        load_text_preview(path)
    } else {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        PreviewContent::Unsupported(filename.to_string())
    }
}

fn load_image_preview(path: &std::path::Path, max_width: u32, max_height: u32) -> PreviewContent {
    match image::open(path) {
        Ok(img) => {
            let (img_w, img_h) = img.dimensions();

            if img_w == 0 || img_h == 0 {
                return PreviewContent::Error("Invalid image dimensions".to_string());
            }

            let scale_x = max_width as f32 / img_w as f32;
            let scale_y = max_height as f32 / img_h as f32;
            let scale = scale_x.min(scale_y).min(1.0);

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
        Err(e) => PreviewContent::Error(format!("Load failed: {}", e)),
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

/// Check if file is an image
pub fn is_image_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg")
    )
}

/// Check if file is SVG
pub fn is_svg_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("svg")
    )
}

/// Check if file is text
pub fn is_text_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some(
            "txt"
                | "md"
                | "rs"
                | "toml"
                | "json"
                | "yaml"
                | "yml"
                | "sh"
                | "py"
                | "js"
                | "ts"
                | "c"
                | "h"
                | "cpp"
                | "hpp"
        )
    )
}

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
        PreviewContent::Unsupported(filename) => {
            let full_rect = Rect::new(0, 0, canvas.width(), canvas.height());
            text_renderer.draw_text_in_rect(
                canvas,
                &format!("No preview for: {}", filename),
                full_rect,
                14.0,
                TextColor::rgb(150, 150, 150),
                HAlign::Center,
                VAlign::Center,
            );
        }
        PreviewContent::Error(msg) => {
            let full_rect = Rect::new(0, 0, canvas.width(), canvas.height());
            text_renderer.draw_text_in_rect(
                canvas,
                msg,
                full_rect,
                14.0,
                TextColor::rgb(200, 100, 100),
                HAlign::Center,
                VAlign::Center,
            );
        }
    }
}

fn render_text_preview(canvas: &mut Canvas, text_renderer: &mut TextRenderer, lines: &[String]) {
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
        text_renderer.draw_text_in_rect(
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
