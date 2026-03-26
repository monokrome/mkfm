//! Media playback handlers

use std::io::Read;
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};

use super::super::App;

/// Active video playback state
pub struct VideoPlayback {
    pub video_child: Child,
    pub video_stdout: std::process::ChildStdout,
    pub audio_child: Option<Child>,
    pub frame_buf: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frame_duration: std::time::Duration,
    pub last_frame: std::time::Instant,
    pub current_frame: Vec<u8>,
    pub playing: bool,
}

impl VideoPlayback {
    /// Read next frame if due. Returns true if a new frame is ready.
    /// Non-blocking — returns false immediately if no data available.
    pub fn advance(&mut self) -> bool {
        if !self.playing {
            return false;
        }

        if self.last_frame.elapsed() < self.frame_duration {
            return false;
        }

        // Non-blocking read — check if data is available first
        let fd = self.video_stdout.as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut pfd, 1, 0) };
        if ready <= 0 {
            return false;
        }

        match self.video_stdout.read_exact(&mut self.frame_buf) {
            Ok(()) => {
                self.current_frame.clear();
                self.current_frame.extend_from_slice(&self.frame_buf);
                self.last_frame = std::time::Instant::now();
                true
            }
            Err(_) => {
                self.playing = false;
                false
            }
        }
    }
}

impl Drop for VideoPlayback {
    fn drop(&mut self) {
        let _ = self.video_child.kill();
        let _ = self.video_child.wait();
        if let Some(ref mut audio) = self.audio_child {
            let _ = audio.kill();
            let _ = audio.wait();
        }
    }
}

impl App {
    pub fn execute_media_play_pause(&mut self) -> bool {
        // If already playing, toggle pause/stop
        if self.playback.is_some() {
            self.playback.take(); // Drop kills processes
            return true;
        }

        let Some(entry) = self.browser().and_then(|b| b.current_entry()) else {
            return false;
        };

        if entry.is_dir || !crate::preview::is_media_file(&entry.path) {
            return false;
        }

        let path = entry.path.clone();
        self.start_playback(&path, 0.0);
        true
    }

    pub fn execute_media_seek(&mut self, seconds: i32) -> bool {
        if self.playback.is_none() {
            return false;
        }

        let Some(path) = self.media_path.clone() else {
            return false;
        };

        self.media_position = (self.media_position + seconds as f64).max(0.0);
        self.playback.take(); // kill current
        self.start_playback(&path, self.media_position);
        true
    }

    pub fn execute_media_zoom(&mut self, zoom_in: bool) -> bool {
        if zoom_in {
            self.preview_width_ratio = (self.preview_width_ratio + 0.05).min(0.8);
        } else {
            self.preview_width_ratio = (self.preview_width_ratio - 0.05).max(0.2);
        }
        true
    }

    fn start_playback(&mut self, path: &std::path::Path, start_seconds: f64) {
        let path_str = path.to_string_lossy().to_string();

        // Get video dimensions and fps
        let (width, height, fps) = match get_video_info(path) {
            Some(info) => info,
            None => return,
        };

        let frame_size = (width * height * 3) as usize;

        // Start video frame pipe
        let mut video_cmd = Command::new("ffmpeg");
        video_cmd.arg("-i").arg(&path_str);
        if start_seconds > 0.0 {
            video_cmd.arg("-ss").arg(format!("{}", start_seconds));
        }
        video_cmd
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-v", "quiet", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        let mut video_child = match video_cmd.spawn() {
            Ok(c) => c,
            Err(_) => return,
        };

        let video_stdout = match video_child.stdout.take() {
            Some(s) => s,
            None => return,
        };

        // Start audio in background (best effort)
        let audio_child = start_audio(&path_str, start_seconds);

        self.playback = Some(VideoPlayback {
            video_child,
            video_stdout,
            audio_child,
            frame_buf: vec![0u8; frame_size],
            width,
            height,
            frame_duration: std::time::Duration::from_secs_f64(1.0 / fps),
            last_frame: std::time::Instant::now(),
            current_frame: Vec::new(),
            playing: true,
        });
        self.media_path = Some(path.to_path_buf());
        self.media_position = start_seconds;
    }
}

fn get_video_info(path: &std::path::Path) -> Option<(u32, u32, f64)> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate",
            "-of", "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let info = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = info.trim().split(',').collect();
    if parts.len() < 3 {
        return None;
    }

    let width: u32 = parts[0].parse().ok()?;
    let height: u32 = parts[1].parse().ok()?;

    let fps_parts: Vec<&str> = parts[2].split('/').collect();
    let fps = if fps_parts.len() == 2 {
        let num: f64 = fps_parts[0].parse().unwrap_or(24.0);
        let den: f64 = fps_parts[1].parse().unwrap_or(1.0);
        num / den
    } else {
        parts[2].parse().unwrap_or(24.0)
    };

    Some((width, height, fps))
}

fn start_audio(path_str: &str, start_seconds: f64) -> Option<Child> {
    for (fmt, device) in [("pulse", "default"), ("alsa", "default")] {
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i").arg(path_str);
        if start_seconds > 0.0 {
            cmd.arg("-ss").arg(format!("{}", start_seconds));
        }
        cmd.args(["-vn", "-f", fmt, device, "-v", "quiet"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Ok(child) = cmd.spawn() {
            return Some(child);
        }
    }
    None
}
