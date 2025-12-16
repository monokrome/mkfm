//! Archive operations (listing, extraction)

use std::path::Path;
use std::process::Command;

/// Archive entry for virtual browsing
#[derive(Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Extract an archive to the destination directory
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let file_name = archive_path.to_string_lossy().to_string();
    let dest_str = dest_dir.to_string_lossy().to_string();

    let (cmd, args): (&str, Vec<String>) = match ext.as_deref() {
        Some("zip") => (
            "unzip",
            vec!["-q".into(), file_name.clone(), "-d".into(), dest_str],
        ),
        Some("gz") if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") => (
            "tar",
            vec!["-xzf".into(), file_name.clone(), "-C".into(), dest_str],
        ),
        Some("bz2") if file_name.ends_with(".tar.bz2") => (
            "tar",
            vec!["-xjf".into(), file_name.clone(), "-C".into(), dest_str],
        ),
        Some("xz") if file_name.ends_with(".tar.xz") => (
            "tar",
            vec!["-xJf".into(), file_name.clone(), "-C".into(), dest_str],
        ),
        Some("tar") => (
            "tar",
            vec!["-xf".into(), file_name.clone(), "-C".into(), dest_str],
        ),
        Some("7z") => (
            "7z",
            vec!["x".into(), format!("-o{}", dest_str), file_name.clone()],
        ),
        Some("rar") => ("unrar", vec!["x".into(), file_name.clone(), dest_str]),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported archive format",
            ));
        }
    };

    let status = Command::new(cmd)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("extraction failed"))
    }
}

/// List contents of an archive
pub fn list_archive(archive_path: &Path) -> Vec<ArchiveEntry> {
    let ext = archive_path
        .extension()
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

    let Ok(output) = output else {
        return Vec::new();
    };
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };

    let mut entries = Vec::new();

    for line in stdout.lines().skip(3) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let size: u64 = parts[0].parse().unwrap_or(0);
            let name = parts[3..].join(" ");
            if !name.is_empty() && name != "Name" {
                let is_dir = name.ends_with('/');
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name
                        .split('/')
                        .next_back()
                        .unwrap_or(&clean_name)
                        .to_string(),
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

    let Ok(output) = output else {
        return Vec::new();
    };
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };

    let mut entries = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let perms = parts[0];
            let is_dir = perms.starts_with('d');
            let size: u64 = parts[2].parse().unwrap_or(0);
            let name = parts[5..].join(" ");
            if !name.is_empty() {
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name
                        .split('/')
                        .next_back()
                        .unwrap_or(&clean_name)
                        .to_string(),
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
        .args(["l", "-slt", path.to_string_lossy().as_ref()])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    let mut current_path = String::new();
    let mut current_size: u64 = 0;
    let mut current_is_dir = false;

    for line in stdout.lines() {
        if let Some(p) = line.strip_prefix("Path = ") {
            if !current_path.is_empty() {
                entries.push(ArchiveEntry {
                    name: current_path
                        .split('/')
                        .next_back()
                        .unwrap_or(&current_path)
                        .to_string(),
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

    if !current_path.is_empty() {
        entries.push(ArchiveEntry {
            name: current_path
                .split('/')
                .next_back()
                .unwrap_or(&current_path)
                .to_string(),
            full_path: current_path,
            is_dir: current_is_dir,
            size: current_size,
        });
    }
    entries
}

fn list_rar(path: &Path) -> Vec<ArchiveEntry> {
    let output = Command::new("unrar")
        .args(["l", path.to_string_lossy().as_ref()])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    let mut in_files = false;

    for line in stdout.lines() {
        if line.contains("-------") {
            in_files = !in_files;
            continue;
        }
        if !in_files {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let is_dir = parts[0].contains('D');
            let size: u64 = parts[1].parse().unwrap_or(0);
            let name = parts[4..].join(" ");
            if !name.is_empty() {
                let clean_name = name.trim_end_matches('/').to_string();
                entries.push(ArchiveEntry {
                    name: clean_name
                        .split('/')
                        .next_back()
                        .unwrap_or(&clean_name)
                        .to_string(),
                    full_path: clean_name,
                    is_dir,
                    size,
                });
            }
        }
    }
    entries
}

/// Extract specific files from an archive
pub fn extract_files_from_archive(
    archive_path: &Path,
    files: &[String],
    dest_dir: &Path,
) -> std::io::Result<()> {
    let ext = archive_path
        .extension()
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
            "unsupported archive format",
        ));
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(std::io::Error::other("extraction failed")),
    }
}
