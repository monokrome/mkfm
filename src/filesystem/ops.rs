//! File operations (copy, move, delete)

use std::fs;
use std::path::Path;

/// Copy a file or directory
pub fn copy_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        copy_directory(src, dest)
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

/// Copy a directory recursively
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

/// Move a file or directory
pub fn move_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::rename(src, dest)
}

/// Delete a file or directory
pub fn delete(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Create a symbolic link
pub fn create_symlink(src: &Path, dest_dir: &Path) -> std::io::Result<()> {
    let name = src
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "No filename"))?;

    let link_path = dest_dir.join(name);

    #[cfg(unix)]
    std::os::unix::fs::symlink(src, &link_path)?;

    #[cfg(windows)]
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, &link_path)?;
    } else {
        std::os::windows::fs::symlink_file(src, &link_path)?;
    }

    Ok(())
}
