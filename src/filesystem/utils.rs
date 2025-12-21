//! Utility functions

use std::path::Path;
use std::process::Command;

/// Format file size as human-readable string
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Move file to trash
pub fn trash(path: &Path) -> std::io::Result<()> {
    // Try trash-put first (trash-cli)
    let result = Command::new("trash-put").arg(path).status();

    if let Ok(status) = result
        && status.success()
    {
        return Ok(());
    }

    // Fall back to gio trash
    let result = Command::new("gio")
        .args(["trash", &path.to_string_lossy()])
        .status();

    if let Ok(status) = result
        && status.success()
    {
        return Ok(());
    }

    // Fall back to moving to XDG trash
    let trash_dir = std::env::var("XDG_DATA_HOME")
        .map(|d| std::path::PathBuf::from(d).join("Trash/files"))
        .or_else(|_| {
            std::env::var("HOME")
                .map(|d| std::path::PathBuf::from(d).join(".local/share/Trash/files"))
        })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;

    std::fs::create_dir_all(&trash_dir)?;

    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "No filename"))?;

    let dest = trash_dir.join(file_name);
    std::fs::rename(path, dest)
}

/// Change file permissions (Unix only)
#[cfg(unix)]
pub fn chmod(path: &Path, mode: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Handle relative modes like +x, -w, etc.
    let current = std::fs::metadata(path)?.permissions();
    let current_mode = current.mode();

    let new_mode = if mode.starts_with('+') || mode.starts_with('-') {
        let add = mode.starts_with('+');
        let bits = mode_char_to_bits(&mode[1..]);

        if add {
            current_mode | bits
        } else {
            current_mode & !bits
        }
    } else if let Ok(octal) = u32::from_str_radix(mode, 8) {
        octal
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid mode",
        ));
    };

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(new_mode))
}

#[cfg(unix)]
fn mode_char_to_bits(chars: &str) -> u32 {
    let mut bits = 0u32;
    for c in chars.chars() {
        match c {
            'r' => bits |= 0o444,
            'w' => bits |= 0o222,
            'x' => bits |= 0o111,
            _ => {}
        }
    }
    bits
}

#[cfg(not(unix))]
pub fn chmod(_path: &Path, _mode: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "chmod not supported on this platform",
    ))
}

/// Check if a file is a supported archive
pub fn is_archive(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let name = path.to_string_lossy().to_lowercase();

    matches!(
        ext.as_deref(),
        Some("zip")
            | Some("tar")
            | Some("gz")
            | Some("bz2")
            | Some("xz")
            | Some("7z")
            | Some("rar")
    ) || name.ends_with(".tar.gz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tar.xz")
        || name.ends_with(".tgz")
}
