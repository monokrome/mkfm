//! Theme configuration

use prefer::Config as PreferConfig;

use super::colors::{parse_rgb, parse_rgba, Rgb, Rgba};

/// Theme configuration
#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Rgba,
    pub foreground: Rgb,
    pub cursor_bg: Rgba,
    pub selection_bg: Rgba,
    pub search_highlight_bg: Rgba,
    pub directory: Rgb,
    pub header_bg: Rgba,
    pub status_bg: Rgba,
    pub border: Rgba,
    pub border_focused: Rgba,
    pub icon_folder: String,
    pub icon_folder_open: String,
    pub icon_file: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Rgba::new(30, 30, 35, 255),
            foreground: Rgb::new(220, 220, 220),
            cursor_bg: Rgba::new(60, 60, 80, 255),
            selection_bg: Rgba::new(80, 60, 60, 255),
            search_highlight_bg: Rgba::new(180, 180, 0, 100),
            directory: Rgb::new(138, 79, 255),
            header_bg: Rgba::new(40, 40, 50, 255),
            status_bg: Rgba::new(50, 50, 60, 255),
            border: Rgba::new(80, 80, 100, 255),
            border_focused: Rgba::new(100, 150, 255, 255),
            icon_folder: "\u{f07b}".to_string(),
            icon_folder_open: "\u{f07c}".to_string(),
            icon_file: "\u{f15b}".to_string(),
        }
    }
}

impl Theme {
    /// Load a theme by name, falling back to default if not found
    pub async fn load(name: Option<&str>) -> Self {
        let Some(name) = name else {
            return Self::default();
        };

        let path = format!("mkfm/themes/{}", name);
        let Ok(config) = prefer::load(&path).await else {
            eprintln!("warning: theme '{}' not found, using default", name);
            return Self::default();
        };

        let mut theme = Self::default();
        theme.load_colors(&config).await;
        theme.load_icons(&config).await;
        theme
    }

    async fn load_colors(&mut self, config: &PreferConfig) {
        if let Some((r, g, b, a)) = get_rgba(config, "background").await {
            self.background = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b)) = get_rgb(config, "foreground").await {
            self.foreground = Rgb::new(r, g, b);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "cursor_bg").await {
            self.cursor_bg = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "selection_bg").await {
            self.selection_bg = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "search_highlight_bg").await {
            self.search_highlight_bg = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b)) = get_rgb(config, "directory").await {
            self.directory = Rgb::new(r, g, b);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "header_bg").await {
            self.header_bg = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "status_bg").await {
            self.status_bg = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "border").await {
            self.border = Rgba::new(r, g, b, a);
        }
        if let Some((r, g, b, a)) = get_rgba(config, "border_focused").await {
            self.border_focused = Rgba::new(r, g, b, a);
        }
    }

    async fn load_icons(&mut self, config: &PreferConfig) {
        if let Some(s) = get_str(config, "icon_folder").await {
            self.icon_folder = s;
        }
        if let Some(s) = get_str(config, "icon_folder_open").await {
            self.icon_folder_open = s;
        }
        if let Some(s) = get_str(config, "icon_file").await {
            self.icon_file = s;
        }
    }
}

async fn get_str(config: &PreferConfig, key: &str) -> Option<String> {
    config
        .get(key)
        .await
        .ok()
        .and_then(|v: prefer::ConfigValue| v.as_str().map(|s| s.to_string()))
}

async fn get_rgb(config: &PreferConfig, key: &str) -> Option<(u8, u8, u8)> {
    get_str(config, key).await.and_then(|s| parse_rgb(&s))
}

async fn get_rgba(config: &PreferConfig, key: &str) -> Option<(u8, u8, u8, u8)> {
    get_str(config, key).await.and_then(|s| parse_rgba(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme.background.r, 30);
        assert_eq!(theme.foreground.r, 220);
        assert_eq!(theme.directory.r, 138);
        assert_eq!(theme.icon_folder, "\u{f07b}");
        assert_eq!(theme.icon_file, "\u{f15b}");
    }
}
