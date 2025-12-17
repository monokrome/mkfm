//! Feature availability initialization

use crate::features::{self, Feature};

use super::App;

impl App {
    /// Initialize feature availability based on mkframe capabilities
    pub fn init_features(
        &mut self,
        has_data_device: bool,
        has_seat: bool,
        has_attached_surface: bool,
    ) {
        self.init_vi_mode_feature();
        self.init_drag_drop_feature(has_data_device, has_seat);
        self.init_preview_feature();
        self.init_overlay_extend_feature(has_attached_surface);
        self.init_archive_feature();
        self.init_trash_feature();
    }

    fn init_vi_mode_feature(&mut self) {
        let desc = if self.vi_mode {
            "Vi-style keybindings (j/k/h/l, gg, G, etc.)"
        } else {
            "Vi-style keybindings (disabled, set vi=true in config)"
        };
        self.feature_list
            .add(Feature::available(features::FEATURE_VI_MODE, desc));
    }

    fn init_drag_drop_feature(&mut self, has_data_device: bool, has_seat: bool) {
        if has_data_device && has_seat {
            self.feature_list.add(Feature::available(
                features::FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
            ));
        } else {
            let reason = if !has_seat {
                "No Wayland seat available."
            } else {
                "Compositor does not support wl_data_device_manager protocol."
            };
            self.feature_list.add(Feature::unavailable(
                features::FEATURE_DRAG_DROP,
                "Drag files to/from other applications",
                reason,
            ));
        }
    }

    fn init_preview_feature(&mut self) {
        self.feature_list.add(Feature::available(
            features::FEATURE_PREVIEW,
            "Preview images and text files in overlay",
        ));
    }

    fn init_overlay_extend_feature(&mut self, has_attached_surface: bool) {
        if has_attached_surface {
            self.feature_list.add(Feature::available(
                features::FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds (wlr-attached-surface)",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                features::FEATURE_OVERLAY_EXTEND,
                "Overlay can extend beyond window bounds",
                "Compositor does not support wlr-attached-surface protocol.",
            ));
        }
    }

    fn init_archive_feature(&mut self) {
        let has_tar = command_exists("tar", &["--version"]);
        let has_unzip = command_exists("unzip", &["-v"]);

        if has_tar || has_unzip {
            self.feature_list.add(Feature::available(
                features::FEATURE_ARCHIVE,
                "Browse and extract archives (tar, zip)",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                features::FEATURE_ARCHIVE,
                "Browse and extract archives (tar, zip)",
                "Neither 'tar' nor 'unzip' commands found in PATH.",
            ));
        }
    }

    fn init_trash_feature(&mut self) {
        let has_trash =
            command_exists("trash-put", &["--version"]) || command_exists("gio", &["help"]);

        if has_trash {
            self.feature_list.add(Feature::available(
                features::FEATURE_TRASH,
                "Move files to trash instead of permanent deletion",
            ));
        } else {
            self.feature_list.add(Feature::unavailable(
                features::FEATURE_TRASH,
                "Move files to trash instead of permanent deletion",
                "Neither 'trash-put' nor 'gio' commands found.",
            ));
        }
    }
}

fn command_exists(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new(cmd).args(args).output().is_ok()
}
