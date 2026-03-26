//! Media playback handlers

use std::process::{Command, Stdio};

use super::super::App;

impl App {
    pub fn execute_media_play_pause(&mut self) -> bool {
        let Some(entry) = self.browser().and_then(|b| b.current_entry()) else {
            return false;
        };

        if entry.is_dir || !crate::preview::is_media_file(&entry.path) {
            return false;
        }

        let path = entry.path.clone();

        if let Some(ref mut child) = self.media_child {
            let _ = child.kill();
            let _ = child.wait();
            self.media_child = None;
            self.media_position = 0.0;
            self.media_path = None;
            return true;
        }

        self.start_media_playback(&path, 0.0);
        true
    }

    pub fn execute_media_seek(&mut self, seconds: i32) -> bool {
        let Some(media_path) = self.media_path.clone() else {
            return false;
        };

        if let Some(ref mut child) = self.media_child {
            let _ = child.kill();
            let _ = child.wait();
        }

        self.media_position = (self.media_position + seconds as f64).max(0.0);
        self.start_media_playback(&media_path, self.media_position);
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

    fn start_media_playback(&mut self, path: &std::path::Path, start_seconds: f64) {
        let path_str = path.to_string_lossy().to_string();

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-i").arg(&path_str);

        if start_seconds > 0.0 {
            cmd.arg("-ss").arg(format!("{}", start_seconds));
        }

        cmd.args(["-vn", "-f", "pulse", "default", "-v", "quiet"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Ok(child) = cmd.spawn() {
            self.media_child = Some(child);
            self.media_path = Some(path.to_path_buf());
        }
    }
}
