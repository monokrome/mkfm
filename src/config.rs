use mkframe::{Color, TextColor};
use prefer::Config as PreferConfig;
use std::io::Write;
use std::path::PathBuf;
use toml::map::Map;

pub struct Config {
    inner: PreferConfig,
}

/// Settings that can be saved to config file
#[derive(Clone, Debug, Default)]
pub struct SavedSettings {
    pub show_hidden: Option<bool>,
    pub show_parent_entry: Option<bool>,
    pub overlay_enabled: Option<bool>,
    pub theme: Option<String>,
    pub vi: Option<bool>,
}

impl SavedSettings {
    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("mkfm").join("config.toml"))
    }

    /// Load existing config as a TOML table
    fn load_existing() -> Map<String, toml::Value> {
        Self::config_path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| s.parse::<toml::Table>().ok())
            .unwrap_or_default()
    }

    /// Save settings to config file, merging with existing config
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "config dir not found")
        })?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Load existing config and merge
        let mut table = Self::load_existing();

        if let Some(v) = self.show_hidden {
            table.insert("show_hidden".to_string(), toml::Value::Boolean(v));
        }
        if let Some(v) = self.show_parent_entry {
            table.insert("show_parent_entry".to_string(), toml::Value::Boolean(v));
        }
        if let Some(v) = self.overlay_enabled {
            // Handle nested overlay.enabled
            let overlay = table
                .entry("overlay".to_string())
                .or_insert_with(|| toml::Value::Table(Map::new()));
            if let toml::Value::Table(t) = overlay {
                t.insert("enabled".to_string(), toml::Value::Boolean(v));
            }
        }
        if let Some(ref v) = self.theme {
            if v.is_empty() {
                table.remove("theme");
            } else {
                table.insert("theme".to_string(), toml::Value::String(v.clone()));
            }
        }
        if let Some(v) = self.vi {
            table.insert("vi".to_string(), toml::Value::Boolean(v));
        }

        // Write to file
        let content = toml::to_string_pretty(&table)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut file = std::fs::File::create(&path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}

/// RGB color for theme
#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_text_color(self) -> TextColor {
        TextColor::rgb(self.r, self.g, self.b)
    }
}

/// RGBA color for theme
#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_color(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

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
            search_highlight_bg: Rgba::new(180, 180, 0, 100), // Yellow highlight
            directory: Rgb::new(138, 79, 255),                // Royal purple
            header_bg: Rgba::new(40, 40, 50, 255),
            status_bg: Rgba::new(50, 50, 60, 255),
            border: Rgba::new(80, 80, 100, 255),
            border_focused: Rgba::new(100, 150, 255, 255),
            icon_folder: "\u{f07b}".to_string(),      //
            icon_folder_open: "\u{f07c}".to_string(), //
            icon_file: "\u{f15b}".to_string(),        //
        }
    }
}

/// Parse color strings like "#1e1e23" or "30,30,35"
fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.starts_with('#') {
        // Hex format
        let hex = s.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b));
        }
    } else if s.contains(',') {
        // CSV format
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            return Some((r, g, b));
        }
    }
    None
}

/// Parse RGBA color strings like "#1e1e23ff" or "30,30,35,255"
fn parse_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let s = s.trim();
    if s.starts_with('#') {
        // Hex format with optional alpha
        let hex = s.trim_start_matches('#');
        if hex.len() == 6 {
            let (r, g, b) = parse_rgb(s)?;
            return Some((r, g, b, 255));
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some((r, g, b, a));
        }
    } else if s.contains(',') {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            let a = if parts.len() >= 4 {
                parts[3].trim().parse().ok()?
            } else {
                255
            };
            return Some((r, g, b, a));
        }
    }
    None
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

        async fn get_str(config: &PreferConfig, key: &str) -> Option<String> {
            config
                .get(key)
                .await
                .ok()
                .and_then(|v: prefer::ConfigValue| v.as_str().map(|s| s.to_string()))
        }

        // Load colors
        if let Some(s) = get_str(&config, "background").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.background = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "foreground").await
            && let Some((r, g, b)) = parse_rgb(&s)
        {
            theme.foreground = Rgb::new(r, g, b);
        }
        if let Some(s) = get_str(&config, "cursor_bg").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.cursor_bg = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "selection_bg").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.selection_bg = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "search_highlight_bg").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.search_highlight_bg = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "directory").await
            && let Some((r, g, b)) = parse_rgb(&s)
        {
            theme.directory = Rgb::new(r, g, b);
        }
        if let Some(s) = get_str(&config, "header_bg").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.header_bg = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "status_bg").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.status_bg = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "border").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.border = Rgba::new(r, g, b, a);
        }
        if let Some(s) = get_str(&config, "border_focused").await
            && let Some((r, g, b, a)) = parse_rgba(&s)
        {
            theme.border_focused = Rgba::new(r, g, b, a);
        }

        // Load icons
        if let Some(s) = get_str(&config, "icon_folder").await {
            theme.icon_folder = s;
        }
        if let Some(s) = get_str(&config, "icon_folder_open").await {
            theme.icon_folder_open = s;
        }
        if let Some(s) = get_str(&config, "icon_file").await {
            theme.icon_file = s;
        }

        theme
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverlayPosition {
    #[default]
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Clone, Debug)]
pub enum Dimension {
    Pixels(i32),
    Percent(f32),
}

impl Dimension {
    pub fn resolve(&self, reference: u32) -> i32 {
        match self {
            Dimension::Pixels(px) => *px,
            Dimension::Percent(pct) => (reference as f32 * pct / 100.0) as i32,
        }
    }

    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(stripped) = s.strip_suffix('%') {
            stripped.parse::<f32>().ok().map(Dimension::Percent)
        } else {
            s.parse::<i32>().ok().map(Dimension::Pixels)
        }
    }
}

#[derive(Clone, Debug)]
pub struct OverlayConfig {
    pub enabled: bool,
    pub margin: Dimension,
    pub position: OverlayPosition,
    pub offset: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            margin: Dimension::Percent(2.0),
            position: OverlayPosition::Right,
            offset: Dimension::Pixels(0),
            max_width: Dimension::Pixels(400),
            max_height: Dimension::Percent(100.0),
        }
    }
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

    /// Whether vim keybindings are enabled (default: false for accessibility)
    pub async fn vi_mode(&self) -> bool {
        self.get_bool("vi").await.unwrap_or(false)
    }

    pub async fn overlay(&self) -> OverlayConfig {
        let mut config = OverlayConfig::default();

        if let Some(enabled) = self.get_bool("overlay.enabled").await {
            config.enabled = enabled;
        }

        if let Some(margin) = self.get_str("overlay.margin").await {
            if let Some(dim) = Dimension::parse(&margin) {
                config.margin = dim;
            }
        } else if let Some(margin) = self.get_i64("overlay.margin").await {
            config.margin = Dimension::Pixels(margin as i32);
        }

        if let Some(pos) = self.get_str("overlay.position").await {
            config.position = match pos.to_lowercase().as_str() {
                "left" => OverlayPosition::Left,
                "right" => OverlayPosition::Right,
                "top" => OverlayPosition::Top,
                "bottom" => OverlayPosition::Bottom,
                _ => OverlayPosition::Right,
            };
        }

        if let Some(offset) = self.get_str("overlay.offset").await {
            if let Some(dim) = Dimension::parse(&offset) {
                config.offset = dim;
            }
        } else if let Some(offset) = self.get_i64("overlay.offset").await {
            config.offset = Dimension::Pixels(offset as i32);
        }

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

        config
    }
}

/// File opener configuration
/// Maps mime type patterns to commands
/// Command can contain {} which will be replaced with file path(s)
#[derive(Clone, Debug, Default)]
pub struct Openers {
    /// Mime pattern -> command template
    /// Patterns can be exact ("image/jpeg") or wildcards ("image/*")
    rules: Vec<(String, String)>,
}

impl Openers {
    /// Load openers from config file
    pub fn load() -> Self {
        let table = SavedSettings::load_existing();
        let mut rules = Vec::new();

        if let Some(toml::Value::Table(openers)) = table.get("openers") {
            for (pattern, value) in openers {
                if let toml::Value::String(cmd) = value {
                    rules.push((pattern.clone(), cmd.clone()));
                }
            }
        }

        // Sort rules: exact matches first (no wildcard), then wildcards
        // This ensures "image/jpeg" is checked before "image/*"
        rules.sort_by(|a, b| {
            let a_wild = a.0.contains('*');
            let b_wild = b.0.contains('*');
            match (a_wild, b_wild) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),
            }
        });

        Self { rules }
    }

    /// Get the opener command for a file path
    /// Returns the command template with {} for file substitution
    pub fn get_opener(&self, path: &std::path::Path) -> String {
        let mime = self.detect_mime(path);

        for (pattern, cmd) in &self.rules {
            if self.matches_pattern(&mime, pattern) {
                return cmd.clone();
            }
        }

        // Default to xdg-open
        "xdg-open {}".to_string()
    }

    /// Detect mime type: use extension if present, magic bytes otherwise
    fn detect_mime(&self, path: &std::path::Path) -> String {
        // If file has an extension, use extension-based detection
        if path.extension().is_some()
            && let Some(mime) = mime_guess::from_path(path).first()
        {
            return mime.to_string();
        }

        // No extension or unknown extension: try magic bytes
        if let Ok(kind) = infer::get_from_path(path)
            && let Some(k) = kind
        {
            return k.mime_type().to_string();
        }

        String::new()
    }

    /// Check if a mime type matches a pattern
    /// Supports exact match and wildcard suffix (e.g., "image/*")
    fn matches_pattern(&self, mime: &str, pattern: &str) -> bool {
        if pattern == mime {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix("/*")
            && let Some(mime_type) = mime.split('/').next()
        {
            return mime_type == prefix;
        }

        false
    }

    /// Execute opener command(s) for a list of files
    /// Groups files by their opener and runs each command once with all matching files
    pub fn open_files(&self, paths: &[std::path::PathBuf]) {
        use std::collections::HashMap;

        // Group files by their command template
        let mut groups: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
        for path in paths {
            if path.is_dir() {
                continue;
            }
            let cmd = self.get_opener(path);
            groups.entry(cmd).or_default().push(path);
        }

        // Execute each command with its files
        for (cmd_template, files) in groups {
            self.execute_command(&cmd_template, &files);
        }
    }

    /// Execute a command template with file paths
    fn execute_command(&self, template: &str, files: &[&std::path::PathBuf]) {
        if files.is_empty() {
            return;
        }

        // Build the file arguments string
        let file_args: Vec<String> = files
            .iter()
            .map(|p| {
                // Quote paths that contain spaces
                let s = p.to_string_lossy();
                if s.contains(' ') {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            })
            .collect();
        let files_str = file_args.join(" ");

        // Replace {} with files, or append if no {}
        let command = if template.contains("{}") {
            template.replace("{}", &files_str)
        } else {
            format!("{} {}", template, files_str)
        };

        // Execute via shell to handle complex commands
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_new() {
        let rgb = Rgb::new(10, 20, 30);
        assert_eq!(rgb.r, 10);
        assert_eq!(rgb.g, 20);
        assert_eq!(rgb.b, 30);
    }

    #[test]
    fn test_rgba_new() {
        let rgba = Rgba::new(10, 20, 30, 40);
        assert_eq!(rgba.r, 10);
        assert_eq!(rgba.g, 20);
        assert_eq!(rgba.b, 30);
        assert_eq!(rgba.a, 40);
    }

    #[test]
    fn test_parse_rgb_hex() {
        assert_eq!(parse_rgb("#1e1e23"), Some((0x1e, 0x1e, 0x23)));
        assert_eq!(parse_rgb("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_rgb("#8a4fff"), Some((138, 79, 255)));
    }

    #[test]
    fn test_parse_rgb_csv() {
        assert_eq!(parse_rgb("30,30,35"), Some((30, 30, 35)));
        assert_eq!(parse_rgb("255, 255, 255"), Some((255, 255, 255)));
        assert_eq!(parse_rgb("  0,  0,  0  "), Some((0, 0, 0)));
    }

    #[test]
    fn test_parse_rgb_invalid() {
        assert_eq!(parse_rgb("#fff"), None); // Too short
        assert_eq!(parse_rgb("#gggggg"), None); // Invalid hex
        assert_eq!(parse_rgb("not,a,color"), None); // Invalid numbers
        assert_eq!(parse_rgb(""), None);
    }

    #[test]
    fn test_parse_rgba_hex() {
        assert_eq!(parse_rgba("#1e1e23ff"), Some((0x1e, 0x1e, 0x23, 0xff)));
        assert_eq!(parse_rgba("#ffffff80"), Some((255, 255, 255, 128)));
        assert_eq!(parse_rgba("#00000000"), Some((0, 0, 0, 0)));
    }

    #[test]
    fn test_parse_rgba_hex_without_alpha() {
        // 6-char hex should default to 255 alpha
        assert_eq!(parse_rgba("#1e1e23"), Some((0x1e, 0x1e, 0x23, 255)));
    }

    #[test]
    fn test_parse_rgba_csv() {
        assert_eq!(parse_rgba("30,30,35,255"), Some((30, 30, 35, 255)));
        assert_eq!(parse_rgba("255, 255, 255, 128"), Some((255, 255, 255, 128)));
    }

    #[test]
    fn test_parse_rgba_csv_without_alpha() {
        // 3-value CSV should default to 255 alpha
        assert_eq!(parse_rgba("30,30,35"), Some((30, 30, 35, 255)));
    }

    #[test]
    fn test_dimension_pixels() {
        let dim = Dimension::Pixels(100);
        assert_eq!(dim.resolve(1000), 100);
        assert_eq!(dim.resolve(500), 100);
    }

    #[test]
    fn test_dimension_percent() {
        let dim = Dimension::Percent(50.0);
        assert_eq!(dim.resolve(1000), 500);
        assert_eq!(dim.resolve(200), 100);
    }

    #[test]
    fn test_dimension_parse_pixels() {
        match Dimension::parse("100") {
            Some(Dimension::Pixels(px)) => assert_eq!(px, 100),
            _ => panic!("expected Pixels(100)"),
        }
        match Dimension::parse("  200  ") {
            Some(Dimension::Pixels(px)) => assert_eq!(px, 200),
            _ => panic!("expected Pixels(200)"),
        }
    }

    #[test]
    fn test_dimension_parse_percent() {
        match Dimension::parse("50%") {
            Some(Dimension::Percent(pct)) => assert!((pct - 50.0).abs() < 0.001),
            _ => panic!("expected Percent(50.0)"),
        }
        match Dimension::parse("  75.5%  ") {
            Some(Dimension::Percent(pct)) => assert!((pct - 75.5).abs() < 0.001),
            _ => panic!("expected Percent(75.5)"),
        }
    }

    #[test]
    fn test_dimension_parse_invalid() {
        assert!(Dimension::parse("abc").is_none());
        assert!(Dimension::parse("").is_none());
    }

    #[test]
    fn test_overlay_position_default() {
        assert_eq!(OverlayPosition::default(), OverlayPosition::Right);
    }

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme.background.r, 30);
        assert_eq!(theme.foreground.r, 220);
        assert_eq!(theme.directory.r, 138); // Royal purple
        assert_eq!(theme.icon_folder, "\u{f07b}");
        assert_eq!(theme.icon_file, "\u{f15b}");
    }

    #[test]
    fn test_overlay_config_default() {
        let config = OverlayConfig::default();
        assert!(config.enabled);
        assert_eq!(config.position, OverlayPosition::Right);
    }

    #[test]
    fn test_saved_settings_default() {
        let settings = SavedSettings::default();
        assert!(settings.show_hidden.is_none());
        assert!(settings.show_parent_entry.is_none());
        assert!(settings.overlay_enabled.is_none());
        assert!(settings.theme.is_none());
    }

    #[test]
    fn test_openers_default() {
        let openers = Openers::default();
        // Default openers should fall back to xdg-open
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.jpg")),
            "xdg-open {}"
        );
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.txt")),
            "xdg-open {}"
        );
    }

    #[test]
    fn test_openers_matches_pattern_exact() {
        let openers = Openers::default();
        assert!(openers.matches_pattern("image/jpeg", "image/jpeg"));
        assert!(!openers.matches_pattern("image/png", "image/jpeg"));
    }

    #[test]
    fn test_openers_matches_pattern_wildcard() {
        let openers = Openers::default();
        assert!(openers.matches_pattern("image/jpeg", "image/*"));
        assert!(openers.matches_pattern("image/png", "image/*"));
        assert!(!openers.matches_pattern("video/mp4", "image/*"));
    }

    #[test]
    fn test_openers_with_rules() {
        let openers = Openers {
            rules: vec![
                ("image/jpeg".to_string(), "feh {}".to_string()),
                ("image/*".to_string(), "imv {}".to_string()),
                ("video/*".to_string(), "mpv {}".to_string()),
            ],
        };

        // Exact match takes priority
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.jpg")),
            "feh {}"
        );
        // Wildcard match
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.png")),
            "imv {}"
        );
        // Different type wildcard
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.mp4")),
            "mpv {}"
        );
        // Fallback to xdg-open
        assert_eq!(
            openers.get_opener(std::path::Path::new("test.xyz")),
            "xdg-open {}"
        );
    }
}
