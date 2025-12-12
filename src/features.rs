/// Feature availability tracking for graceful degradation

#[derive(Clone, Debug)]
pub struct Feature {
    pub name: &'static str,
    pub available: bool,
    pub reason: Option<String>,
    pub description: &'static str,
}

impl Feature {
    pub fn available(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            available: true,
            reason: None,
            description,
        }
    }

    pub fn unavailable(name: &'static str, description: &'static str, reason: impl Into<String>) -> Self {
        Self {
            name,
            available: false,
            reason: Some(reason.into()),
            description,
        }
    }
}

pub struct FeatureList {
    pub features: Vec<Feature>,
}

impl FeatureList {
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }

    pub fn add(&mut self, feature: Feature) {
        self.features.push(feature);
    }

    pub fn is_available(&self, name: &str) -> bool {
        self.features
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.available)
            .unwrap_or(false)
    }

    pub fn available_count(&self) -> usize {
        self.features.iter().filter(|f| f.available).count()
    }

    pub fn unavailable_count(&self) -> usize {
        self.features.iter().filter(|f| !f.available).count()
    }
}

pub struct FeatureListPane {
    pub cursor: usize,
    pub visible: bool,
    pub scroll_offset: usize,
    pub showing_detail: bool,
}

impl FeatureListPane {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            visible: false,
            scroll_offset: 0,
            showing_detail: false,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.showing_detail = false;
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.showing_detail = false;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.showing_detail = false;
    }

    pub fn toggle_detail(&mut self) {
        self.showing_detail = !self.showing_detail;
    }

    pub fn move_cursor(&mut self, delta: i32, max: usize) {
        if max == 0 {
            return;
        }
        if delta > 0 {
            self.cursor = (self.cursor + delta as usize).min(max - 1);
        } else if delta < 0 {
            self.cursor = self.cursor.saturating_sub((-delta) as usize);
        }
    }
}

// Feature name constants
pub const FEATURE_DRAG_DROP: &str = "Drag & Drop";
pub const FEATURE_CLIPBOARD: &str = "Clipboard";
pub const FEATURE_PREVIEW: &str = "File Preview";
pub const FEATURE_ARCHIVE: &str = "Archive Support";
pub const FEATURE_TRASH: &str = "Trash";
pub const FEATURE_VIM_MODE: &str = "Vim Mode";
