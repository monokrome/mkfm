//! Archive extraction functions

use std::path::Path;
use std::process::Command;

use super::ArchiveFormat;

/// Extract an archive to the destination directory
pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let format = ArchiveFormat::detect(archive_path);
    let file = archive_path.to_string_lossy().to_string();
    let dest = dest_dir.to_string_lossy().to_string();

    let (cmd, args) = build_extract_args(format, &file, &dest)?;
    run_extract_command(cmd, &args)
}

fn build_extract_args(
    format: ArchiveFormat,
    file: &str,
    dest: &str,
) -> std::io::Result<(&'static str, Vec<String>)> {
    let result = match format {
        ArchiveFormat::Zip => (
            "unzip",
            vec!["-q".into(), file.into(), "-d".into(), dest.into()],
        ),
        ArchiveFormat::TarGz => (
            "tar",
            vec!["-xzf".into(), file.into(), "-C".into(), dest.into()],
        ),
        ArchiveFormat::TarBz2 => (
            "tar",
            vec!["-xjf".into(), file.into(), "-C".into(), dest.into()],
        ),
        ArchiveFormat::TarXz => (
            "tar",
            vec!["-xJf".into(), file.into(), "-C".into(), dest.into()],
        ),
        ArchiveFormat::Tar => (
            "tar",
            vec!["-xf".into(), file.into(), "-C".into(), dest.into()],
        ),
        ArchiveFormat::SevenZip => ("7z", vec!["x".into(), format!("-o{}", dest), file.into()]),
        ArchiveFormat::Rar => ("unrar", vec!["x".into(), file.into(), dest.into()]),
        ArchiveFormat::Unknown => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported archive format",
            ));
        }
    };
    Ok(result)
}

fn run_extract_command(cmd: &str, args: &[String]) -> std::io::Result<()> {
    let status = Command::new(cmd)
        .args(args)
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

/// Extract specific files from an archive
pub fn extract_files_from_archive(
    archive_path: &Path,
    files: &[String],
    dest_dir: &Path,
) -> std::io::Result<()> {
    let format = ArchiveFormat::detect(archive_path);
    let archive = archive_path.to_string_lossy().to_string();
    let dest = dest_dir.to_string_lossy().to_string();

    let status = run_selective_extract(format, &archive, &dest, files)?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("extraction failed"))
    }
}

fn run_selective_extract(
    format: ArchiveFormat,
    archive: &str,
    dest: &str,
    files: &[String],
) -> std::io::Result<std::process::ExitStatus> {
    match format {
        ArchiveFormat::Zip => {
            let mut args = vec!["-q".to_string(), archive.into(), "-d".into(), dest.into()];
            args.extend(files.iter().cloned());
            Command::new("unzip").args(&args).status()
        }
        ArchiveFormat::TarGz => run_tar_extract("-xzf", archive, dest, files),
        ArchiveFormat::TarBz2 => run_tar_extract("-xjf", archive, dest, files),
        ArchiveFormat::TarXz => run_tar_extract("-xJf", archive, dest, files),
        ArchiveFormat::Tar => run_tar_extract("-xf", archive, dest, files),
        ArchiveFormat::SevenZip => {
            let mut args = vec!["x".into(), format!("-o{}", dest), archive.into()];
            args.extend(files.iter().cloned());
            Command::new("7z").args(&args).status()
        }
        ArchiveFormat::Rar => {
            let mut args = vec!["x".to_string(), archive.into(), dest.into()];
            args.extend(files.iter().cloned());
            Command::new("unrar").args(&args).status()
        }
        ArchiveFormat::Unknown => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsupported archive format",
        )),
    }
}

fn run_tar_extract(
    flag: &str,
    archive: &str,
    dest: &str,
    files: &[String],
) -> std::io::Result<std::process::ExitStatus> {
    let mut args = vec![flag.to_string(), archive.into(), "-C".into(), dest.into()];
    args.extend(files.iter().cloned());
    Command::new("tar").args(&args).status()
}
