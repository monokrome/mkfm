//! Preview content caching

use std::path::PathBuf;

use super::PreviewContent;
use super::loaders::load_preview_content;

/// Cache for loaded preview content
pub struct PreviewCache {
    path: Option<PathBuf>,
    content: Option<PreviewContent>,
    cached_width: u32,
    cached_height: u32,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self {
            path: None,
            content: None,
            cached_width: 0,
            cached_height: 0,
        }
    }

    pub fn get_or_load(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> &PreviewContent {
        let path_changed = self.path.as_deref() != Some(path);
        let dims_changed = self.cached_width != max_width || self.cached_height != max_height;

        if path_changed || dims_changed {
            self.path = Some(path.to_path_buf());
            self.cached_width = max_width;
            self.cached_height = max_height;
            self.content = Some(self.load_content(path, max_width, max_height));
        }
        self.content.as_ref().unwrap()
    }

    fn load_content(
        &mut self,
        path: &std::path::Path,
        max_width: u32,
        max_height: u32,
    ) -> PreviewContent {
        let path_owned = path.to_path_buf();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            load_preview_content(&path_owned, max_width, max_height)
        }))
        .unwrap_or_else(|_| PreviewContent::Error("Preview crashed".to_string()))
    }

    pub fn invalidate(&mut self) {
        self.path = None;
        self.content = None;
    }
}
