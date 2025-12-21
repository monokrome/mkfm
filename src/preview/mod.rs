//! Preview content loading and rendering
//!
//! Split into submodules to reduce complexity.

mod cache;
mod loaders;
mod render;

use std::time::Duration;

pub use cache::PreviewCache;
pub use render::render_preview;

/// Media file metadata
#[derive(Clone, Debug, Default)]
pub struct MediaMetadata {
    pub duration: Option<Duration>,
    pub codec: Option<String>,
    pub bitrate: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl MediaMetadata {
    /// Format duration as HH:MM:SS or MM:SS
    pub fn format_duration(&self) -> Option<String> {
        self.duration.map(|d| {
            let secs = d.as_secs();
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, mins, secs)
            } else {
                format!("{:02}:{:02}", mins, secs)
            }
        })
    }
}

/// Media type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    Audio,
    Video,
}

/// Playback state for media preview
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Paused,
    Playing,
}

/// Content types for file preview
pub enum PreviewContent {
    Image {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    Text(Vec<String>),
    Media {
        media_type: MediaType,
        metadata: MediaMetadata,
        thumbnail: Option<Vec<u8>>,
        thumb_width: u32,
        thumb_height: u32,
    },
    Unsupported(String),
    Error(String),
}

impl PreviewContent {
    pub fn dimensions(&self, max_width: u32, max_height: u32) -> (u32, u32) {
        match self {
            PreviewContent::Image { width, height, .. } => (*width, *height),
            PreviewContent::Text(lines) => calculate_text_dimensions(lines, max_width, max_height),
            PreviewContent::Media {
                thumb_width,
                thumb_height,
                metadata,
                ..
            } => calculate_media_dimensions(
                *thumb_width,
                *thumb_height,
                metadata,
                max_width,
                max_height,
            ),
            PreviewContent::Unsupported(_) | PreviewContent::Error(_) => (200, 50),
        }
    }
}

fn calculate_text_dimensions(lines: &[String], max_width: u32, max_height: u32) -> (u32, u32) {
    let line_height = 16u32;
    let char_width = 8u32;
    let max_line_len = lines.iter().map(|l| l.len().min(80)).max().unwrap_or(20) as u32;
    let height = (lines.len() as u32 * line_height).min(max_height);
    let width = (max_line_len * char_width).min(max_width);
    (width.max(100), height.max(50))
}

fn calculate_media_dimensions(
    thumb_width: u32,
    thumb_height: u32,
    metadata: &MediaMetadata,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let info_height = 80u32;
    let base_w = if thumb_width > 0 { thumb_width } else { 300 };
    let base_h = if thumb_height > 0 {
        thumb_height + info_height
    } else {
        info_height + 50
    };
    let w = base_w.min(max_width).max(200);
    let h = if metadata.width.is_some() {
        base_h.min(max_height)
    } else {
        (info_height + 20).min(max_height)
    };
    (w, h)
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

/// Check if file is audio
pub fn is_audio_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" | "opus" | "aiff")
    )
}

/// Check if file is video
pub fn is_video_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" | "mpeg" | "mpg")
    )
}

/// Check if file is any media type (audio or video)
pub fn is_media_file(path: &std::path::Path) -> bool {
    is_audio_file(path) || is_video_file(path)
}
