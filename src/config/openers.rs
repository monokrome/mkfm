//! File opener configuration

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::saved::SavedSettings;

/// File opener configuration
/// Maps mime type patterns to commands
#[derive(Clone, Debug, Default)]
pub struct Openers {
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

        Self::sort_rules(&mut rules);
        Self { rules }
    }

    fn sort_rules(rules: &mut [(String, String)]) {
        rules.sort_by(|a, b| {
            let a_wild = a.0.contains('*');
            let b_wild = b.0.contains('*');
            match (a_wild, b_wild) {
                (false, true) => std::cmp::Ordering::Less,
                (true, false) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),
            }
        });
    }

    /// Get the opener command for a file path
    pub fn get_opener(&self, path: &Path) -> String {
        let mime = self.detect_mime(path);

        for (pattern, cmd) in &self.rules {
            if self.matches_pattern(&mime, pattern) {
                return cmd.clone();
            }
        }

        "xdg-open {}".to_string()
    }

    fn detect_mime(&self, path: &Path) -> String {
        if path.extension().is_some()
            && let Some(mime) = mime_guess::from_path(path).first()
        {
            return mime.to_string();
        }

        if let Ok(kind) = infer::get_from_path(path)
            && let Some(k) = kind
        {
            return k.mime_type().to_string();
        }

        String::new()
    }

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
    pub fn open_files(&self, paths: &[PathBuf]) {
        let mut groups: HashMap<String, Vec<&PathBuf>> = HashMap::new();
        for path in paths {
            if path.is_dir() {
                continue;
            }
            let cmd = self.get_opener(path);
            groups.entry(cmd).or_default().push(path);
        }

        for (cmd_template, files) in groups {
            self.execute_command(&cmd_template, &files);
        }
    }

    fn execute_command(&self, template: &str, files: &[&PathBuf]) {
        if files.is_empty() {
            return;
        }

        let files_str = Self::build_file_args(files);
        let command = Self::substitute_template(template, &files_str);

        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    fn build_file_args(files: &[&PathBuf]) -> String {
        files
            .iter()
            .map(|p| {
                let s = p.to_string_lossy();
                if s.contains(' ') {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn substitute_template(template: &str, files_str: &str) -> String {
        if template.contains("{}") {
            template.replace("{}", files_str)
        } else {
            format!("{} {}", template, files_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openers_default() {
        let openers = Openers::default();
        assert_eq!(openers.get_opener(Path::new("test.jpg")), "xdg-open {}");
        assert_eq!(openers.get_opener(Path::new("test.txt")), "xdg-open {}");
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

        assert_eq!(openers.get_opener(Path::new("test.jpg")), "feh {}");
        assert_eq!(openers.get_opener(Path::new("test.png")), "imv {}");
        assert_eq!(openers.get_opener(Path::new("test.mp4")), "mpv {}");
        assert_eq!(openers.get_opener(Path::new("test.xyz")), "xdg-open {}");
    }
}
