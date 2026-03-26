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
    /// Accumulation buffer for partial reads
    read_buf: Vec<u8>,
    /// Bytes read so far into the current frame
    read_pos: usize,
    /// Size of one frame in bytes
    frame_size: usize,
    pub width: u32,
    pub height: u32,
    pub frame_duration: std::time::Duration,
    pub last_frame: std::time::Instant,
    pub current_frame: Vec<u8>,
    pub playing: bool,
}

impl VideoPlayback {
    /// Advance to the latest available frame. Fully non-blocking.
    pub fn advance(&mut self) -> bool {
        if !self.playing {
            return false;
        }

        let fd = self.video_stdout.as_raw_fd();
        let mut got_frame = false;

        loop {
            // Check if data is available without blocking
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut pfd, 1, 0) };
            if ready <= 0 {
                break;
            }

            // Read whatever is available (non-blocking partial read)
            let remaining = self.frame_size - self.read_pos;
            let buf = &mut self.read_buf[self.read_pos..self.read_pos + remaining];
            match self.video_stdout.read(buf) {
                Ok(0) => {
                    // EOF — video ended
                    self.playing = false;
                    break;
                }
                Ok(n) => {
                    self.read_pos += n;

                    // Complete frame?
                    if self.read_pos >= self.frame_size {
                        self.current_frame.clear();
                        self.current_frame.extend_from_slice(&self.read_buf[..self.frame_size]);
                        self.read_pos = 0;
                        got_frame = true;

                        // If caught up to real time, stop and render this frame
                        if self.last_frame.elapsed() < self.frame_duration * 2 {
                            break;
                        }
                        // Otherwise keep reading to skip ahead
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.playing = false;
                    break;
                }
            }
        }

        if got_frame {
            self.last_frame = std::time::Instant::now();
        }

        got_frame
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
        if self.playback.is_some() {
            self.playback.take();
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
        self.playback.take();
        self.start_playback(&path, self.media_position);
        true
    }

    /// Zoom preview. `amount` is a percentage (e.g. 70 = set to 70% of pane).
    /// Without a count, nudges by 5%.
    pub fn execute_media_zoom(&mut self, zoom_in: bool, amount: Option<usize>) -> bool {
        if let Some(pct) = amount {
            // Absolute: 70gmk = preview takes 70% of pane
            self.preview_zoom = Some((pct as f32 / 100.0).clamp(0.1, 0.9));
        } else if zoom_in {
            let current = self.preview_zoom.unwrap_or(0.5);
            self.preview_zoom = Some((current + 0.05).min(0.9));
        } else {
            let current = self.preview_zoom.unwrap_or(0.5);
            self.preview_zoom = Some((current - 0.05).max(0.1));
        }
        true
    }

    fn start_playback(&mut self, path: &std::path::Path, start_seconds: f64) {
        let path_str = path.to_string_lossy().to_string();

        let (orig_width, orig_height, fps) = match get_video_info(path) {
            Some(info) => info,
            None => return,
        };

        // Downscale to a reasonable size for terminal preview
        // Max 320 pixels wide — keeps Kitty data manageable over SSH
        let max_px = 320u32;
        let (width, height) = if orig_width > max_px || orig_height > max_px {
            let scale = max_px as f32 / orig_width.max(orig_height) as f32;
            let w = ((orig_width as f32 * scale) as u32) & !1; // even dimensions
            let h = ((orig_height as f32 * scale) as u32) & !1;
            (w.max(2), h.max(2))
        } else {
            (orig_width & !1, orig_height & !1)
        };

        let frame_size = (width * height * 3) as usize;
        let scale_filter = format!("scale={}:{}", width, height);

        let mut video_cmd = Command::new("ffmpeg");
        video_cmd.arg("-i").arg(&path_str);
        if start_seconds > 0.0 {
            video_cmd.arg("-ss").arg(format!("{}", start_seconds));
        }
        video_cmd
            .args(["-vf", &scale_filter, "-f", "rawvideo", "-pix_fmt", "rgb24", "-v", "quiet", "-"])
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

        // Set pipe to non-blocking so read() never blocks
        let fd = video_stdout.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        let audio_child = start_audio(&path_str, start_seconds);

        self.playback = Some(VideoPlayback {
            video_child,
            video_stdout,
            audio_child,
            read_buf: vec![0u8; frame_size],
            read_pos: 0,
            frame_size,
            width,
            height,
            // Cap at 10fps for terminal rendering — Kitty protocol is bandwidth-heavy
            frame_duration: std::time::Duration::from_secs_f64(1.0 / fps.min(10.0)),
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
    // Try pulse first, then alsa, then skip if no audio available
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
