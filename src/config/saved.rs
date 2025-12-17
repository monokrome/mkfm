//! Settings that can be saved to config file

use std::io::Write;
use std::path::PathBuf;
use toml::map::Map;

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
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("mkfm").join("config.toml"))
    }

    /// Load existing config as a TOML table
    pub fn load_existing() -> Map<String, toml::Value> {
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

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut table = Self::load_existing();
        self.apply_to_table(&mut table);

        let content = toml::to_string_pretty(&table)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut file = std::fs::File::create(&path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    fn apply_to_table(&self, table: &mut Map<String, toml::Value>) {
        if let Some(v) = self.show_hidden {
            table.insert("show_hidden".to_string(), toml::Value::Boolean(v));
        }
        if let Some(v) = self.show_parent_entry {
            table.insert("show_parent_entry".to_string(), toml::Value::Boolean(v));
        }
        if let Some(v) = self.overlay_enabled {
            self.set_overlay_enabled(table, v);
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
    }

    fn set_overlay_enabled(&self, table: &mut Map<String, toml::Value>, enabled: bool) {
        let overlay = table
            .entry("overlay".to_string())
            .or_insert_with(|| toml::Value::Table(Map::new()));
        if let toml::Value::Table(t) = overlay {
            t.insert("enabled".to_string(), toml::Value::Boolean(enabled));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_settings_default() {
        let settings = SavedSettings::default();
        assert!(settings.show_hidden.is_none());
        assert!(settings.show_parent_entry.is_none());
        assert!(settings.overlay_enabled.is_none());
        assert!(settings.theme.is_none());
    }
}
