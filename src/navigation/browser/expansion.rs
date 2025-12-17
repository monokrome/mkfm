//! Directory fold/expansion methods

use std::path::Path;

use crate::filesystem::{self, Entry};

use super::expansion_helpers::{
    expand_all_marked, filter_hidden, find_children_range, get_expandable_entry, insert_children,
    load_children, remove_children,
};
use super::Browser;

impl Browser {
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_dirs.contains(path)
    }

    pub fn expand_directory(&mut self, index: usize, recursive: bool) {
        let Some(entry) = get_expandable_entry(&self.entries, index) else {
            return;
        };
        if self.expanded_dirs.contains(&entry.path) {
            return;
        }

        self.expanded_dirs.insert(entry.path.clone());
        let children =
            load_children(&entry.path, entry.depth + 1, self.show_hidden, self.sort_mode, self.sort_reverse);
        insert_children(&mut self.entries, index + 1, children, recursive, &mut self.expanded_dirs);

        if recursive {
            self.rebuild_with_expansions();
        }
    }

    pub fn collapse_directory(&mut self, index: usize, recursive: bool) {
        let Some(entry) = get_expandable_entry(&self.entries, index) else {
            return;
        };
        if !self.expanded_dirs.contains(&entry.path) {
            return;
        }

        let range = find_children_range(&self.entries, index);
        remove_children(&mut self.entries, range.clone(), recursive, &mut self.expanded_dirs);
        self.expanded_dirs.remove(&entry.path);
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    pub fn toggle_expansion(&mut self, index: usize, recursive: bool) {
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        if !entry.is_dir || entry.name == ".." {
            return;
        }

        if self.expanded_dirs.contains(&entry.path) {
            self.collapse_directory(index, recursive);
        } else {
            self.expand_directory(index, recursive);
        }
    }

    fn rebuild_with_expansions(&mut self) {
        let mut new_entries = self.load_base_entries();
        expand_all_marked(
            &mut new_entries,
            &self.expanded_dirs,
            self.show_hidden,
            self.sort_mode,
            self.sort_reverse,
        );
        self.entries = new_entries.clone();
        self.all_entries = new_entries;
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    fn load_base_entries(&self) -> Vec<Entry> {
        let mut entries = filesystem::list_directory(&self.path);
        filter_hidden(&mut entries, self.show_hidden, self.show_parent_entry);
        Self::sort_entries_impl(&mut entries, self.sort_mode, self.sort_reverse);
        entries
    }
}
