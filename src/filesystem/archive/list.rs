//! Archive listing functions

use std::path::Path;
use std::process::Command;

use super::{ArchiveEntry, ArchiveFormat};

/// List contents of an archive
pub fn list_archive(archive_path: &Path) -> Vec<ArchiveEntry> {
    match ArchiveFormat::detect(archive_path) {
        ArchiveFormat::Zip => list_zip(archive_path),
        ArchiveFormat::TarGz => list_tar(archive_path, "z"),
        ArchiveFormat::TarBz2 => list_tar(archive_path, "j"),
        ArchiveFormat::TarXz => list_tar(archive_path, "J"),
        ArchiveFormat::Tar => list_tar(archive_path, ""),
        ArchiveFormat::SevenZip => list_7z(archive_path),
        ArchiveFormat::Rar => list_rar(archive_path),
        ArchiveFormat::Unknown => Vec::new(),
    }
}

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    String::from_utf8(output.stdout).ok()
}

fn list_zip(path: &Path) -> Vec<ArchiveEntry> {
    let path_str = path.to_string_lossy();
    let Some(stdout) = run_command("unzip", &["-l", &path_str]) else {
        return Vec::new();
    };

    stdout
        .lines()
        .skip(3)
        .filter_map(parse_zip_line)
        .collect()
}

fn parse_zip_line(line: &str) -> Option<ArchiveEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let size: u64 = parts[0].parse().unwrap_or(0);
    let name = parts[3..].join(" ");
    if name.is_empty() || name == "Name" {
        return None;
    }

    let is_dir = name.ends_with('/');
    let clean_name = name.trim_end_matches('/').to_string();
    Some(ArchiveEntry::new(clean_name, is_dir, size))
}

fn list_tar(path: &Path, compression: &str) -> Vec<ArchiveEntry> {
    let flag = format!("-t{}vf", compression);
    let path_str = path.to_string_lossy().to_string();
    let Some(stdout) = run_command("tar", &[&flag, &path_str]) else {
        return Vec::new();
    };

    stdout.lines().filter_map(parse_tar_line).collect()
}

fn parse_tar_line(line: &str) -> Option<ArchiveEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }

    let perms = parts[0];
    let is_dir = perms.starts_with('d');
    let size: u64 = parts[2].parse().unwrap_or(0);
    let name = parts[5..].join(" ");
    if name.is_empty() {
        return None;
    }

    let clean_name = name.trim_end_matches('/').to_string();
    Some(ArchiveEntry::new(clean_name, is_dir, size))
}

fn list_7z(path: &Path) -> Vec<ArchiveEntry> {
    let path_str = path.to_string_lossy();
    let Some(stdout) = run_command("7z", &["l", "-slt", &path_str]) else {
        return Vec::new();
    };

    parse_7z_output(&stdout)
}

fn parse_7z_output(stdout: &str) -> Vec<ArchiveEntry> {
    let mut entries = Vec::new();
    let mut current = SevenZipEntry::default();

    for line in stdout.lines() {
        if let Some(p) = line.strip_prefix("Path = ") {
            if !current.path.is_empty() {
                entries.push(current.to_archive_entry());
            }
            current = SevenZipEntry {
                path: p.to_string(),
                ..Default::default()
            };
        } else if let Some(s) = line.strip_prefix("Size = ") {
            current.size = s.parse().unwrap_or(0);
        } else if line.starts_with("Attributes = D") {
            current.is_dir = true;
        }
    }

    if !current.path.is_empty() {
        entries.push(current.to_archive_entry());
    }
    entries
}

#[derive(Default)]
struct SevenZipEntry {
    path: String,
    size: u64,
    is_dir: bool,
}

impl SevenZipEntry {
    fn to_archive_entry(&self) -> ArchiveEntry {
        ArchiveEntry::new(self.path.clone(), self.is_dir, self.size)
    }
}

fn list_rar(path: &Path) -> Vec<ArchiveEntry> {
    let path_str = path.to_string_lossy();
    let Some(stdout) = run_command("unrar", &["l", &path_str]) else {
        return Vec::new();
    };

    parse_rar_output(&stdout)
}

fn parse_rar_output(stdout: &str) -> Vec<ArchiveEntry> {
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
        if let Some(entry) = parse_rar_line(line) {
            entries.push(entry);
        }
    }
    entries
}

fn parse_rar_line(line: &str) -> Option<ArchiveEntry> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }

    let is_dir = parts[0].contains('D');
    let size: u64 = parts[1].parse().unwrap_or(0);
    let name = parts[4..].join(" ");
    if name.is_empty() {
        return None;
    }

    let clean_name = name.trim_end_matches('/').to_string();
    Some(ArchiveEntry::new(clean_name, is_dir, size))
}
