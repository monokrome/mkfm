//! Helper functions for directory expansion

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::filesystem::{self, Entry};
use crate::input::SortMode;

use super::Browser;

pub fn get_expandable_entry(entries: &[Entry], index: usize) -> Option<Entry> {
    let entry = entries.get(index)?.clone();
    if !entry.is_dir || entry.name == ".." {
        return None;
    }
    Some(entry)
}

pub fn insert_children(
    entries: &mut Vec<Entry>,
    insert_pos: usize,
    children: Vec<Entry>,
    recursive: bool,
    expanded_dirs: &mut HashSet<PathBuf>,
) {
    for (i, child) in children.into_iter().enumerate() {
        if recursive && child.is_dir && child.name != ".." {
            expanded_dirs.insert(child.path.clone());
        }
        entries.insert(insert_pos + i, child);
    }
}

pub fn remove_children(
    entries: &mut Vec<Entry>,
    range: std::ops::Range<usize>,
    recursive: bool,
    expanded_dirs: &mut HashSet<PathBuf>,
) {
    if recursive {
        for i in range.clone() {
            if let Some(child) = entries.get(i)
                && child.is_dir
            {
                expanded_dirs.remove(&child.path);
            }
        }
    }
    if !range.is_empty() {
        entries.drain(range);
    }
}

pub fn find_children_range(entries: &[Entry], index: usize) -> std::ops::Range<usize> {
    let parent_depth = entries.get(index).map(|e| e.depth).unwrap_or(0);
    let start = index + 1;
    let end = entries[start..]
        .iter()
        .position(|e| e.depth <= parent_depth)
        .map(|p| start + p)
        .unwrap_or(entries.len());
    start..end
}

pub fn load_children(
    dir_path: &Path,
    depth: u8,
    show_hidden: bool,
    sort_mode: SortMode,
    sort_reverse: bool,
) -> Vec<Entry> {
    let mut children = filesystem::list_directory(dir_path);
    children.retain(|e| e.name != "..");
    for entry in &mut children {
        entry.depth = depth;
    }
    if !show_hidden {
        children.retain(|e| !e.name.starts_with('.'));
    }
    Browser::sort_entries_impl(&mut children, sort_mode, sort_reverse);
    children
}

pub fn filter_hidden(entries: &mut Vec<Entry>, show_hidden: bool, show_parent_entry: bool) {
    if !show_hidden {
        entries.retain(|e| !e.name.starts_with('.') || (show_parent_entry && e.name == ".."));
    }
}

pub fn expand_all_marked(
    entries: &mut Vec<Entry>,
    expanded_dirs: &HashSet<PathBuf>,
    show_hidden: bool,
    sort_mode: SortMode,
    sort_reverse: bool,
) {
    let mut i = 0;
    while i < entries.len() {
        if should_expand_entry(&entries[i], expanded_dirs) {
            let children = load_children(
                &entries[i].path,
                entries[i].depth + 1,
                show_hidden,
                sort_mode,
                sort_reverse,
            );
            for (j, child) in children.into_iter().enumerate() {
                entries.insert(i + 1 + j, child);
            }
        }
        i += 1;
    }
}

fn should_expand_entry(entry: &Entry, expanded_dirs: &HashSet<PathBuf>) -> bool {
    entry.is_dir && entry.name != ".." && expanded_dirs.contains(&entry.path)
}
