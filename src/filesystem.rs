use std::path::{Path, PathBuf};
use std::fs::{self, DirEntry};
use std::cmp::Ordering;
use std::time::SystemTime;
use std::process::Command;

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl Entry {
    fn from_dir_entry(entry: DirEntry) -> Option<Self> {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = path.is_dir();
        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata.and_then(|m| m.modified().ok());

        Some(Self { name, path, is_dir, size, modified })
    }
}

pub fn list_directory(path: &Path) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut entries: Vec<Entry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(Entry::from_dir_entry)
        .collect();

    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    if let Some(parent) = path.parent() {
        entries.insert(0, Entry {
            name: "..".to_string(),
            path: parent.to_path_buf(),
            is_dir: true,
            size: 0,
            modified: None,
        });
    }

    entries
}

pub fn copy_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        copy_directory(src, dest)
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

pub fn copy_directory(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_directory(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

pub fn move_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::rename(src, dest)
}

pub fn delete(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    match bytes {
        b if b >= GB => format!("{:.1}G", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1}M", b as f64 / MB as f64),
        b if b >= KB => format!("{:.1}K", b as f64 / KB as f64),
        b => format!("{}B", b),
    }
}

/// Move file to trash following freedesktop.org trash specification
pub fn trash(path: &Path) -> std::io::Result<()> {
    let trash_dir = dirs::data_dir()
        .map(|d| d.join("Trash"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "trash dir not found"))?;

    let files_dir = trash_dir.join("files");
    let info_dir = trash_dir.join("info");

    fs::create_dir_all(&files_dir)?;
    fs::create_dir_all(&info_dir)?;

    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid file name"))?;

    // Generate unique name if file exists in trash
    let mut dest_name = file_name.to_string();
    let mut counter = 1;
    while files_dir.join(&dest_name).exists() {
        let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
        let ext = Path::new(file_name).extension().and_then(|e| e.to_str());
        dest_name = match ext {
            Some(e) => format!("{}.{}.{}", stem, counter, e),
            None => format!("{}.{}", stem, counter),
        };
        counter += 1;
    }

    // Write .trashinfo file
    let info_path = info_dir.join(format!("{}.trashinfo", dest_name));
    let deletion_date = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let original_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    // Note: freedesktop spec uses ISO 8601, but timestamp works for basic functionality
    let info_content = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}",
        original_path.display(),
        deletion_date
    );
    fs::write(&info_path, info_content)?;

    // Move file to trash
    let dest_path = files_dir.join(&dest_name);
    fs::rename(path, &dest_path).or_else(|_| {
        // If rename fails (cross-device), copy and delete
        copy_file(path, &dest_path)?;
        delete(path)
    })
}

/// Extract an archive to the destination directory
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let file_name = archive_path.to_string_lossy().to_string();
    let dest_str = dest_dir.to_string_lossy().to_string();

    let (cmd, args): (&str, Vec<String>) = match ext.as_deref() {
        Some("zip") => ("unzip", vec!["-q".into(), file_name.clone(), "-d".into(), dest_str.clone()]),
        Some("gz") if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") => {
            ("tar", vec!["-xzf".into(), file_name.clone(), "-C".into(), dest_str.clone()])
        }
        Some("bz2") if file_name.ends_with(".tar.bz2") => {
            ("tar", vec!["-xjf".into(), file_name.clone(), "-C".into(), dest_str.clone()])
        }
        Some("xz") if file_name.ends_with(".tar.xz") => {
            ("tar", vec!["-xJf".into(), file_name.clone(), "-C".into(), dest_str.clone()])
        }
        Some("tar") => ("tar", vec!["-xf".into(), file_name.clone(), "-C".into(), dest_str.clone()]),
        Some("7z") => ("7z", vec!["x".into(), format!("-o{}", dest_str), file_name.clone()]),
        Some("rar") => ("unrar", vec!["x".into(), file_name.clone(), dest_str.clone()]),
        _ => return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsupported archive format"
        )),
    };

    let status = std::process::Command::new(cmd)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "extraction failed"))
    }
}

/// Change file permissions (Unix only)
#[cfg(unix)]
pub fn chmod(path: &Path, mode: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Parse mode - support octal (755) or symbolic (+x, -w, etc.)
    let mode_u32 = if mode.chars().all(|c| c.is_ascii_digit()) {
        // Octal mode
        u32::from_str_radix(mode, 8)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid mode"))?
    } else {
        // For now, just support octal. Symbolic mode parsing is complex.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "only octal modes supported (e.g., 755)"
        ));
    };

    let permissions = fs::Permissions::from_mode(mode_u32);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
pub fn chmod(_path: &Path, _mode: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "chmod not supported on this platform"
    ))
}

/// Create a symbolic link
pub fn create_symlink(src: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let file_name = src.file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid source path"))?;
    let dest = dest_dir.join(file_name);

    #[cfg(unix)]
    std::os::unix::fs::symlink(src, dest)?;

    #[cfg(windows)]
    {
        if src.is_dir() {
            std::os::windows::fs::symlink_dir(src, dest)?;
        } else {
            std::os::windows::fs::symlink_file(src, dest)?;
        }
    }

    Ok(())
}

/// Archive entry for virtual browsing
#[derive(Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub full_path: String,  // Path within archive
    pub is_dir: bool,
    pub size: u64,
}

/// Check if a file is a supported archive
pub fn is_archive(path: &Path) -> bool {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let name = path.to_string_lossy().to_lowercase();

    matches!(ext.as_deref(), Some("zip") | Some("tar") | Some("gz") | Some("bz2") | Some("xz") | Some("7z") | Some("rar"))
        || name.ends_with(".tar.gz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tar.xz")
        || name.ends_with(".tgz")
}

/// List contents of an archive
pub fn list_archive(archive_path: &Path) -> Vec<ArchiveEntry> {
    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let name = archive_path.to_string_lossy().to_lowercase();

    if ext.as_deref() == Some("zip") {
        list_zip(archive_path)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") || ext.as_deref() == Some("gz") {
        list_tar(archive_path, "z")
    } else if name.ends_with(".tar.bz2") || ext.as_deref() == Some("bz2") {
        list_tar(archive_path, "j")
    } else if name.ends_with(".tar.xz") || ext.as_deref() == Some("xz") {
        list_tar(archive_path, "J")
    } else if ext.as_deref() == Some("tar") {
        list_tar(archive_path, "")
    } else if ext.as_deref() == Some("7z") {
        list_7z(archive_path)
    } else if ext.as_deref() == Some("rar") {
        list_rar(archive_path)
    } else {
        Vec::new()
    }
}

fn list_zip(path: &Path) -> Vec<ArchiveEntry> {
    let output = Command::new("unzip")
        .args(["-l", &path.to_string_lossy()])
        .output();

    let Ok(output) = output else { return Vec::new() };
    let Ok(stdout) = String::from_utf8(output.stdout) else { return Vec::new() };

    let mut entries = Vec::new();

    // Parse unzip -l output (skip header and footer)
    for line in stdout.lines().skip(3) {
        // Format: "  Length      Date    Time    Name"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let size: u64 = parts[0].parse().unwrap_or(0);
            let name = parts[3..].join(" ");
            if !name.is_empty() && name != "Name" {
                let is_dir = name.ends_with('/');
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name.split('/').last().unwrap_or(&clean_name).to_string(),
                    full_path: clean_name,
                    is_dir,
                    size,
                });
            }
        }
    }

    entries
}

fn list_tar(path: &Path, compression: &str) -> Vec<ArchiveEntry> {
    let flag = format!("-t{}vf", compression);
    let output = Command::new("tar")
        .args([&flag, &path.to_string_lossy().to_string()])
        .output();

    let Ok(output) = output else { return Vec::new() };
    let Ok(stdout) = String::from_utf8(output.stdout) else { return Vec::new() };

    let mut entries = Vec::new();

    for line in stdout.lines() {
        // Format: "-rw-r--r-- user/group  size date time name"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let perms = parts[0];
            let is_dir = perms.starts_with('d');
            let size: u64 = parts[2].parse().unwrap_or(0);
            let name = parts[5..].join(" ");
            if !name.is_empty() {
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name.split('/').last().unwrap_or(&clean_name).to_string(),
                    full_path: clean_name,
                    is_dir,
                    size,
                });
            }
        }
    }

    entries
}

fn list_7z(path: &Path) -> Vec<ArchiveEntry> {
    let output = Command::new("7z")
        .args(["l", "-slt", &path.to_string_lossy().to_string()])
        .output();

    let Ok(output) = output else { return Vec::new() };
    let Ok(stdout) = String::from_utf8(output.stdout) else { return Vec::new() };

    let mut entries = Vec::new();
    let mut current_path = String::new();
    let mut current_size: u64 = 0;
    let mut current_is_dir = false;

    for line in stdout.lines() {
        if let Some(p) = line.strip_prefix("Path = ") {
            if !current_path.is_empty() {
                entries.push(ArchiveEntry {
                    name: current_path.split('/').last().unwrap_or(&current_path).to_string(),
                    full_path: current_path.clone(),
                    is_dir: current_is_dir,
                    size: current_size,
                });
            }
            current_path = p.to_string();
            current_size = 0;
            current_is_dir = false;
        } else if let Some(s) = line.strip_prefix("Size = ") {
            current_size = s.parse().unwrap_or(0);
        } else if line.starts_with("Attributes = D") {
            current_is_dir = true;
        }
    }

    // Don't forget the last entry
    if !current_path.is_empty() {
        entries.push(ArchiveEntry {
            name: current_path.split('/').last().unwrap_or(&current_path).to_string(),
            full_path: current_path,
            is_dir: current_is_dir,
            size: current_size,
        });
    }

    entries
}

fn list_rar(path: &Path) -> Vec<ArchiveEntry> {
    let output = Command::new("unrar")
        .args(["l", &path.to_string_lossy().to_string()])
        .output();

    let Ok(output) = output else { return Vec::new() };
    let Ok(stdout) = String::from_utf8(output.stdout) else { return Vec::new() };

    let mut entries = Vec::new();

    // Parse unrar l output - skip header lines
    let mut in_files = false;
    for line in stdout.lines() {
        if line.contains("-------") {
            in_files = !in_files;
            continue;
        }
        if !in_files { continue; }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            // Format varies but typically: Attributes Size Packed Ratio Date Time Name
            let is_dir = parts[0].contains('D');
            let size: u64 = parts[1].parse().unwrap_or(0);
            let name = parts[4..].join(" ");
            if !name.is_empty() {
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name.split('/').last().unwrap_or(&clean_name).to_string(),
                    full_path: clean_name,
                    is_dir,
                    size,
                });
            }
        }
    }

    entries
}

/// Extract specific files from an archive to a destination
pub fn extract_files_from_archive(
    archive_path: &Path,
    files: &[String],
    dest_dir: &Path,
) -> std::io::Result<()> {
    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let name = archive_path.to_string_lossy().to_lowercase();
    let archive_str = archive_path.to_string_lossy().to_string();
    let dest_str = dest_dir.to_string_lossy().to_string();

    let status = if ext.as_deref() == Some("zip") {
        let mut args = vec!["-q".to_string(), archive_str, "-d".to_string(), dest_str];
        args.extend(files.iter().cloned());
        Command::new("unzip").args(&args).status()
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") || ext.as_deref() == Some("gz") {
        let mut args = vec!["-xzf".to_string(), archive_str, "-C".to_string(), dest_str];
        args.extend(files.iter().cloned());
        Command::new("tar").args(&args).status()
    } else if name.ends_with(".tar.bz2") || ext.as_deref() == Some("bz2") {
        let mut args = vec!["-xjf".to_string(), archive_str, "-C".to_string(), dest_str];
        args.extend(files.iter().cloned());
        Command::new("tar").args(&args).status()
    } else if name.ends_with(".tar.xz") || ext.as_deref() == Some("xz") {
        let mut args = vec!["-xJf".to_string(), archive_str, "-C".to_string(), dest_str];
        args.extend(files.iter().cloned());
        Command::new("tar").args(&args).status()
    } else if ext.as_deref() == Some("tar") {
        let mut args = vec!["-xf".to_string(), archive_str, "-C".to_string(), dest_str];
        args.extend(files.iter().cloned());
        Command::new("tar").args(&args).status()
    } else if ext.as_deref() == Some("7z") {
        let mut args = vec!["x".to_string(), format!("-o{}", dest_str), archive_str];
        args.extend(files.iter().cloned());
        Command::new("7z").args(&args).status()
    } else if ext.as_deref() == Some("rar") {
        let mut args = vec!["x".to_string(), archive_str, dest_str];
        args.extend(files.iter().cloned());
        Command::new("unrar").args(&args).status()
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsupported archive format"
        ));
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "extraction failed")),
    }
}
