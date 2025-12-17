//! Entry sorting logic

use std::cmp::Ordering;

use crate::filesystem::Entry;
use crate::input::SortMode;

use super::Browser;

impl Browser {
    pub(super) fn sort_entries_impl(
        entries: &mut [Entry],
        sort_mode: SortMode,
        sort_reverse: bool,
    ) {
        let start = skip_parent_entry(entries);
        let slice = &mut entries[start..];

        match sort_mode {
            SortMode::Name => slice.sort_by(compare_by_name),
            SortMode::Size => slice.sort_by(compare_by_size),
            SortMode::Date => slice.sort_by(compare_by_date),
            SortMode::Type => slice.sort_by(compare_by_type),
        }

        if sort_reverse {
            slice.reverse();
        }
    }
}

fn skip_parent_entry(entries: &[Entry]) -> usize {
    if entries.first().map(|e| e.name == "..").unwrap_or(false) {
        1
    } else {
        0
    }
}

fn dirs_first(a: &Entry, b: &Entry) -> Option<Ordering> {
    match (a.is_dir, b.is_dir) {
        (true, false) => Some(Ordering::Less),
        (false, true) => Some(Ordering::Greater),
        _ => None,
    }
}

fn compare_by_name(a: &Entry, b: &Entry) -> Ordering {
    dirs_first(a, b).unwrap_or_else(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

fn compare_by_size(a: &Entry, b: &Entry) -> Ordering {
    dirs_first(a, b).unwrap_or_else(|| a.size.cmp(&b.size))
}

fn compare_by_date(a: &Entry, b: &Entry) -> Ordering {
    dirs_first(a, b).unwrap_or_else(|| a.modified.cmp(&b.modified))
}

fn compare_by_type(a: &Entry, b: &Entry) -> Ordering {
    dirs_first(a, b).unwrap_or_else(|| {
        let ext_a = a.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_b = b.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
    })
}
