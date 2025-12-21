//! FFmpeg CLI backend for media metadata extraction

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::preview::{MediaMetadata, MediaType, PreviewContent};

/// Check if ffprobe is available
pub fn is_available() -> bool {
    Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Load media preview using ffprobe for metadata
pub fn load_media_preview(path: &Path, max_width: u32, max_height: u32) -> PreviewContent {
    let media_type = if crate::preview::is_video_file(path) {
        MediaType::Video
    } else {
        MediaType::Audio
    };

    let metadata = match extract_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return PreviewContent::Error(format!("Failed to read metadata: {}", e));
        }
    };

    let (thumbnail, thumb_width, thumb_height) = if media_type == MediaType::Video {
        extract_thumbnail(path, max_width, max_height)
    } else {
        (None, 0, 0)
    };

    PreviewContent::Media {
        media_type,
        metadata,
        thumbnail,
        thumb_width,
        thumb_height,
    }
}

fn extract_metadata(path: &Path) -> Result<MediaMetadata, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("ffprobe failed".to_string());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    parse_ffprobe_json(&json_str)
}

fn parse_ffprobe_json(json: &str) -> Result<MediaMetadata, String> {
    let mut metadata = MediaMetadata::default();

    // Parse duration from format section
    if let Some(duration_str) = extract_json_value(json, "duration")
        && let Ok(secs) = duration_str.parse::<f64>()
    {
        metadata.duration = Some(Duration::from_secs_f64(secs));
    }

    // Parse bitrate from format section
    if let Some(bitrate_str) = extract_json_value(json, "bit_rate")
        && let Ok(br) = bitrate_str.parse::<u64>()
    {
        metadata.bitrate = Some(br);
    }

    // Parse tags (title, artist, album)
    metadata.title = extract_tag(json, "title");
    metadata.artist = extract_tag(json, "artist").or_else(|| extract_tag(json, "album_artist"));
    metadata.album = extract_tag(json, "album");

    // Parse stream info
    parse_stream_info(json, &mut metadata);

    Ok(metadata)
}

fn parse_stream_info(json: &str, metadata: &mut MediaMetadata) {
    // Find video stream dimensions
    if let Some(width_str) = extract_json_value(json, "width")
        && let Ok(w) = width_str.parse::<u32>()
    {
        metadata.width = Some(w);
    }
    if let Some(height_str) = extract_json_value(json, "height")
        && let Ok(h) = height_str.parse::<u32>()
    {
        metadata.height = Some(h);
    }

    // Parse codec
    metadata.codec = extract_json_value(json, "codec_name").map(|s| s.to_string());

    // Parse audio info
    if let Some(sr) = extract_json_value(json, "sample_rate")
        && let Ok(rate) = sr.parse::<u32>()
    {
        metadata.sample_rate = Some(rate);
    }
    if let Some(ch) = extract_json_value(json, "channels")
        && let Ok(channels) = ch.parse::<u8>()
    {
        metadata.channels = Some(channels);
    }
}

fn extract_json_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let after_key = after_key.trim_start();

    if let Some(content) = after_key.strip_prefix('"') {
        // String value
        let end = content.find('"')?;
        Some(&content[..end])
    } else {
        // Number or other value
        let end = after_key.find([',', '}', '\n'])?;
        Some(after_key[..end].trim())
    }
}

fn extract_tag(json: &str, tag_name: &str) -> Option<String> {
    // Look for tag in "tags" section
    let tags_start = json.find("\"tags\"")?;
    let tags_section = &json[tags_start..];
    let tag_end = tags_section.find('}')?;
    let tags_content = &tags_section[..tag_end];

    extract_json_value(tags_content, tag_name).map(|s| s.to_string())
}

fn extract_thumbnail(path: &Path, max_width: u32, max_height: u32) -> (Option<Vec<u8>>, u32, u32) {
    // Use ffmpeg to extract a frame at 1 second (or start if shorter)
    let scale_filter = format!(
        "scale='min({},iw)':min'({},ih)':force_original_aspect_ratio=decrease",
        max_width, max_height
    );

    let output = Command::new("ffmpeg")
        .args(["-ss", "1", "-i"])
        .arg(path)
        .args([
            "-vframes",
            "1",
            "-vf",
            &scale_filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() && !out.stdout.is_empty() => {
            // We need to know the actual dimensions from ffprobe
            let (w, h) = get_frame_dimensions(path, max_width, max_height);
            if w > 0 && h > 0 {
                (Some(out.stdout), w, h)
            } else {
                (None, 0, 0)
            }
        }
        _ => (None, 0, 0),
    }
}

fn get_frame_dimensions(path: &Path, max_width: u32, max_height: u32) -> (u32, u32) {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output();

    if let Ok(out) = output {
        let s = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = s.trim().split(',').collect();
        if parts.len() == 2
            && let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
        {
            // Scale to fit max dimensions
            let scale_x = max_width as f32 / w as f32;
            let scale_y = max_height as f32 / h as f32;
            let scale = scale_x.min(scale_y).min(1.0);
            return ((w as f32 * scale) as u32, (h as f32 * scale) as u32);
        }
    }
    (0, 0)
}
