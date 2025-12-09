use prefer::Config as PreferConfig;

pub struct Config {
    inner: PreferConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayPosition {
    Right,
    Left,
    Top,
    Bottom,
}

impl Default for OverlayPosition {
    fn default() -> Self {
        Self::Right
    }
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
        if s.ends_with('%') {
            s[..s.len() - 1].parse::<f32>().ok().map(Dimension::Percent)
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
        self.inner.get(key).await.ok().and_then(|v: prefer::ConfigValue| v.as_bool())
    }

    async fn get_i64(&self, key: &str) -> Option<i64> {
        self.inner.get(key).await.ok().and_then(|v: prefer::ConfigValue| v.as_i64())
    }

    async fn get_str(&self, key: &str) -> Option<String> {
        self.inner.get(key).await.ok().and_then(|v: prefer::ConfigValue| v.as_str().map(|s| s.to_string()))
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

