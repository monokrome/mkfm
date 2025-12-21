//! Configuration module
//!
//! Split into submodules for reduced complexity.

mod colors;
mod openers;
mod overlay;
mod saved;
mod theme;

pub use openers::Openers;
pub use overlay::{Dimension, OverlayConfig, OverlayPosition};
pub use saved::SavedSettings;
pub use theme::Theme;

/// Icon display mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconsMode {
    #[default]
    Auto,
    Enabled,
    Disabled,
}

use prefer::Config as PreferConfig;

pub struct Config {
    inner: PreferConfig,
}

impl Config {
    pub async fn load() -> prefer::Result<Self> {
        let inner = prefer::load("mkfm/config").await?;
        Ok(Self { inner })
    }

    async fn get_bool(&self, key: &str) -> Option<bool> {
        self.inner
            .get(key)
            .await
            .ok()
            .and_then(|v: prefer::ConfigValue| v.as_bool())
    }

    async fn get_i64(&self, key: &str) -> Option<i64> {
        self.inner
            .get(key)
            .await
            .ok()
            .and_then(|v: prefer::ConfigValue| v.as_i64())
    }

    async fn get_str(&self, key: &str) -> Option<String> {
        self.inner
            .get(key)
            .await
            .ok()
            .and_then(|v: prefer::ConfigValue| v.as_str().map(|s| s.to_string()))
    }

    pub async fn show_hidden(&self) -> bool {
        self.get_bool("show_hidden").await.unwrap_or(false)
    }

    pub async fn show_parent_entry(&self) -> bool {
        self.get_bool("show_parent_entry").await.unwrap_or(true)
    }

    pub async fn start_in_cwd(&self) -> bool {
        self.get_bool("start_in_cwd").await.unwrap_or(true)
    }

    pub async fn decorations(&self) -> bool {
        self.get_bool("decorations").await.unwrap_or(true)
    }

    pub async fn theme(&self) -> Option<String> {
        self.get_str("theme").await
    }

    pub async fn vi_mode(&self) -> bool {
        self.get_bool("vi").await.unwrap_or(false)
    }

    pub async fn search_narrowing(&self) -> bool {
        self.get_bool("search_narrowing").await.unwrap_or(false)
    }

    pub async fn icons(&self) -> IconsMode {
        match self.get_str("icons").await.as_deref() {
            Some("true") | Some("enabled") | Some("on") => IconsMode::Enabled,
            Some("false") | Some("disabled") | Some("off") => IconsMode::Disabled,
            Some("auto") | None => IconsMode::Auto,
            _ => IconsMode::Auto,
        }
    }

    pub async fn overlay(&self) -> OverlayConfig {
        let mut config = OverlayConfig::default();
        self.load_overlay_enabled(&mut config).await;
        self.load_overlay_margin(&mut config).await;
        self.load_overlay_position(&mut config).await;
        self.load_overlay_offset(&mut config).await;
        self.load_overlay_dimensions(&mut config).await;
        config
    }

    async fn load_overlay_enabled(&self, config: &mut OverlayConfig) {
        if let Some(enabled) = self.get_bool("overlay.enabled").await {
            config.enabled = enabled;
        }
    }

    async fn load_overlay_margin(&self, config: &mut OverlayConfig) {
        if let Some(margin) = self.get_str("overlay.margin").await {
            if let Some(dim) = Dimension::parse(&margin) {
                config.margin = dim;
            }
        } else if let Some(margin) = self.get_i64("overlay.margin").await {
            config.margin = Dimension::Pixels(margin as i32);
        }
    }

    async fn load_overlay_position(&self, config: &mut OverlayConfig) {
        if let Some(pos) = self.get_str("overlay.position").await {
            config.position = match pos.to_lowercase().as_str() {
                "left" => OverlayPosition::Left,
                "right" => OverlayPosition::Right,
                "top" => OverlayPosition::Top,
                "bottom" => OverlayPosition::Bottom,
                _ => OverlayPosition::Right,
            };
        }
    }

    async fn load_overlay_offset(&self, config: &mut OverlayConfig) {
        if let Some(offset) = self.get_str("overlay.offset").await {
            if let Some(dim) = Dimension::parse(&offset) {
                config.offset = dim;
            }
        } else if let Some(offset) = self.get_i64("overlay.offset").await {
            config.offset = Dimension::Pixels(offset as i32);
        }
    }

    async fn load_overlay_dimensions(&self, config: &mut OverlayConfig) {
        if let Some(w) = self.get_str("overlay.max_width").await {
            if let Some(dim) = Dimension::parse(&w) {
                config.max_width = dim;
            }
        } else if let Some(w) = self.get_i64("overlay.max_width").await {
            config.max_width = Dimension::Pixels(w as i32);
        }

        if let Some(h) = self.get_str("overlay.max_height").await {
            if let Some(dim) = Dimension::parse(&h) {
                config.max_height = dim;
            }
        } else if let Some(h) = self.get_i64("overlay.max_height").await {
            config.max_height = Dimension::Pixels(h as i32);
        }
    }
}
