//! Split tree layout system for dividing a window into multiple panes.

use mkui::layout::Rect;

/// Unique identifier for a leaf node in the split tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeafId(pub usize);

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Split horizontally (top/bottom).
    Horizontal,
    /// Split vertically (left/right).
    Vertical,
}

/// A node in the split tree.
#[derive(Debug)]
enum SplitNode<T> {
    Leaf { id: LeafId, content: T },
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<SplitNode<T>>,
        second: Box<SplitNode<T>>,
    },
}

/// A tree of splits managing layout and focus.
pub struct SplitTree<T> {
    root: Option<SplitNode<T>>,
    focused: Option<LeafId>,
    next_id: usize,
}

impl<T> Default for SplitTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SplitTree<T> {
    pub fn new() -> Self {
        Self {
            root: None,
            focused: None,
            next_id: 0,
        }
    }

    pub fn with_root(content: T) -> Self {
        let mut tree = Self::new();
        tree.set_root(content);
        tree
    }

    fn next_leaf_id(&mut self) -> LeafId {
        let id = LeafId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn set_root(&mut self, content: T) -> LeafId {
        let id = self.next_leaf_id();
        self.root = Some(SplitNode::Leaf { id, content });
        self.focused = Some(id);
        id
    }

    pub fn split_vertical(&mut self, content: T) -> Option<LeafId> {
        self.split_focused(SplitDirection::Vertical, content)
    }

    pub fn split_horizontal(&mut self, content: T) -> Option<LeafId> {
        self.split_focused(SplitDirection::Horizontal, content)
    }

    fn split_focused(&mut self, direction: SplitDirection, content: T) -> Option<LeafId> {
        let focused_id = self.focused?;
        let new_id = self.next_leaf_id();

        self.root = self
            .root
            .take()
            .map(|node| Self::split_node(node, focused_id, direction, new_id, content));

        self.focused = Some(new_id);
        Some(new_id)
    }

    fn split_node(
        node: SplitNode<T>,
        target_id: LeafId,
        direction: SplitDirection,
        new_id: LeafId,
        content: T,
    ) -> SplitNode<T> {
        match node {
            SplitNode::Leaf {
                id,
                content: old_content,
            } if id == target_id => SplitNode::Split {
                direction,
                ratio: 0.5,
                first: Box::new(SplitNode::Leaf {
                    id,
                    content: old_content,
                }),
                second: Box::new(SplitNode::Leaf {
                    id: new_id,
                    content,
                }),
            },
            SplitNode::Leaf { .. } => node,
            SplitNode::Split {
                direction: d,
                ratio,
                first,
                second,
            } => {
                if Self::node_contains_leaf(&first, target_id) {
                    SplitNode::Split {
                        direction: d,
                        ratio,
                        first: Box::new(Self::split_node(
                            *first, target_id, direction, new_id, content,
                        )),
                        second,
                    }
                } else {
                    SplitNode::Split {
                        direction: d,
                        ratio,
                        first,
                        second: Box::new(Self::split_node(
                            *second, target_id, direction, new_id, content,
                        )),
                    }
                }
            }
        }
    }

    pub fn focused(&self) -> Option<LeafId> {
        self.focused
    }

    pub fn set_focused(&mut self, id: LeafId) {
        if self.contains_leaf(id) {
            self.focused = Some(id);
        }
    }

    pub fn contains_leaf(&self, id: LeafId) -> bool {
        self.root
            .as_ref()
            .is_some_and(|n| Self::node_contains_leaf(n, id))
    }

    fn node_contains_leaf(node: &SplitNode<T>, id: LeafId) -> bool {
        match node {
            SplitNode::Leaf { id: leaf_id, .. } => *leaf_id == id,
            SplitNode::Split { first, second, .. } => {
                Self::node_contains_leaf(first, id) || Self::node_contains_leaf(second, id)
            }
        }
    }

    pub fn focused_content(&self) -> Option<&T> {
        let focused_id = self.focused?;
        self.get(focused_id)
    }

    pub fn focused_content_mut(&mut self) -> Option<&mut T> {
        let focused_id = self.focused?;
        self.get_mut(focused_id)
    }

    pub fn get(&self, id: LeafId) -> Option<&T> {
        self.root.as_ref().and_then(|n| Self::node_get(n, id))
    }

    fn node_get(node: &SplitNode<T>, id: LeafId) -> Option<&T> {
        match node {
            SplitNode::Leaf {
                id: leaf_id,
                content,
            } if *leaf_id == id => Some(content),
            SplitNode::Leaf { .. } => None,
            SplitNode::Split { first, second, .. } => {
                Self::node_get(first, id).or_else(|| Self::node_get(second, id))
            }
        }
    }

    pub fn get_mut(&mut self, id: LeafId) -> Option<&mut T> {
        self.root.as_mut().and_then(|n| Self::node_get_mut(n, id))
    }

    fn node_get_mut(node: &mut SplitNode<T>, id: LeafId) -> Option<&mut T> {
        match node {
            SplitNode::Leaf {
                id: leaf_id,
                content,
            } if *leaf_id == id => Some(content),
            SplitNode::Leaf { .. } => None,
            SplitNode::Split { first, second, .. } => {
                Self::node_get_mut(first, id).or_else(|| Self::node_get_mut(second, id))
            }
        }
    }

    pub fn len(&self) -> usize {
        self.root.as_ref().map_or(0, Self::node_len)
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    fn node_len(node: &SplitNode<T>) -> usize {
        match node {
            SplitNode::Leaf { .. } => 1,
            SplitNode::Split { first, second, .. } => {
                Self::node_len(first) + Self::node_len(second)
            }
        }
    }

    pub fn layout(&self, bounds: Rect) -> Vec<(LeafId, Rect)> {
        let mut result = Vec::new();
        if let Some(ref node) = self.root {
            Self::layout_node(node, bounds, &mut result);
        }
        result
    }

    fn layout_node(node: &SplitNode<T>, bounds: Rect, result: &mut Vec<(LeafId, Rect)>) {
        match node {
            SplitNode::Leaf { id, .. } => {
                result.push((*id, bounds));
            }
            SplitNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) = Self::split_bounds(bounds, *direction, *ratio);
                Self::layout_node(first, first_bounds, result);
                Self::layout_node(second, second_bounds, result);
            }
        }
    }

    pub fn find_at_position(&self, bounds: Rect, x: u16, y: u16) -> Option<(LeafId, Rect)> {
        let layout = self.layout(bounds);
        for (id, rect) in layout {
            if rect.contains(x, y) {
                return Some((id, rect));
            }
        }
        None
    }

    fn split_bounds(bounds: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
        match direction {
            SplitDirection::Vertical => {
                let first_width = (bounds.width as f32 * ratio) as u16;
                let second_width = bounds.width.saturating_sub(first_width);
                (
                    Rect::new(bounds.x, bounds.y, first_width, bounds.height),
                    Rect::new(
                        bounds.x + first_width,
                        bounds.y,
                        second_width,
                        bounds.height,
                    ),
                )
            }
            SplitDirection::Horizontal => {
                let first_height = (bounds.height as f32 * ratio) as u16;
                let second_height = bounds.height.saturating_sub(first_height);
                (
                    Rect::new(bounds.x, bounds.y, bounds.width, first_height),
                    Rect::new(
                        bounds.x,
                        bounds.y + first_height,
                        bounds.width,
                        second_height,
                    ),
                )
            }
        }
    }

    pub fn render<F>(&self, bounds: Rect, mut render_fn: F)
    where
        F: FnMut(LeafId, Rect, &T, bool),
    {
        let focused = self.focused;
        if let Some(ref node) = self.root {
            Self::render_node(node, bounds, focused, &mut render_fn);
        }
    }

    fn render_node<F>(node: &SplitNode<T>, bounds: Rect, focused: Option<LeafId>, render_fn: &mut F)
    where
        F: FnMut(LeafId, Rect, &T, bool),
    {
        match node {
            SplitNode::Leaf { id, content } => {
                let is_focused = focused == Some(*id);
                render_fn(*id, bounds, content, is_focused);
            }
            SplitNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) = Self::split_bounds(bounds, *direction, *ratio);
                Self::render_node(first, first_bounds, focused, render_fn);
                Self::render_node(second, second_bounds, focused, render_fn);
            }
        }
    }

    pub fn focus_direction(&mut self, direction: SplitDirection, forward: bool) -> bool {
        let Some(focused_id) = self.focused else {
            return false;
        };
        if self.root.is_none() {
            return false;
        };

        let bounds = Rect::new(0, 0, 1000, 1000);
        let layout = self.layout(bounds);

        let Some((_, focused_rect)) = layout.iter().find(|(id, _)| *id == focused_id) else {
            return false;
        };

        let focused_center_x = focused_rect.x + focused_rect.width / 2;
        let focused_center_y = focused_rect.y + focused_rect.height / 2;

        let mut best: Option<(LeafId, u16)> = None;

        for (id, rect) in &layout {
            if *id == focused_id {
                continue;
            }

            let center_x = rect.x + rect.width / 2;
            let center_y = rect.y + rect.height / 2;

            let is_valid = match (direction, forward) {
                (SplitDirection::Horizontal, true) => center_y > focused_center_y,
                (SplitDirection::Horizontal, false) => center_y < focused_center_y,
                (SplitDirection::Vertical, true) => center_x > focused_center_x,
                (SplitDirection::Vertical, false) => center_x < focused_center_x,
            };

            if !is_valid {
                continue;
            }

            let distance = match direction {
                SplitDirection::Horizontal => center_y.abs_diff(focused_center_y),
                SplitDirection::Vertical => center_x.abs_diff(focused_center_x),
            };

            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((*id, distance));
            }
        }

        if let Some((id, _)) = best {
            self.focused = Some(id);
            true
        } else {
            false
        }
    }

    pub fn focus_left(&mut self) -> bool {
        self.focus_direction(SplitDirection::Vertical, false)
    }

    pub fn focus_right(&mut self) -> bool {
        self.focus_direction(SplitDirection::Vertical, true)
    }

    pub fn focus_up(&mut self) -> bool {
        self.focus_direction(SplitDirection::Horizontal, false)
    }

    pub fn focus_down(&mut self) -> bool {
        self.focus_direction(SplitDirection::Horizontal, true)
    }

    pub fn close_focused(&mut self) -> Option<T> {
        let focused_id = self.focused?;
        let (new_root, removed, new_focus) = Self::remove_leaf(self.root.take()?, focused_id)?;
        self.root = new_root;
        self.focused = new_focus;
        Some(removed)
    }

    fn remove_leaf(
        node: SplitNode<T>,
        target: LeafId,
    ) -> Option<(Option<SplitNode<T>>, T, Option<LeafId>)> {
        match node {
            SplitNode::Leaf { id, content } if id == target => Some((None, content, None)),
            SplitNode::Leaf { .. } => None,
            SplitNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first_contains = Self::node_contains_leaf(&first, target);
                let second_contains = Self::node_contains_leaf(&second, target);

                if first_contains {
                    let (new_first, removed, _) = Self::remove_leaf(*first, target)?;
                    let new_focus = Self::first_leaf_id(&second);
                    match new_first {
                        Some(f) => Some((
                            Some(SplitNode::Split {
                                direction,
                                ratio,
                                first: Box::new(f),
                                second,
                            }),
                            removed,
                            new_focus,
                        )),
                        None => Some((Some(*second), removed, new_focus)),
                    }
                } else if second_contains {
                    let (new_second, removed, _) = Self::remove_leaf(*second, target)?;
                    let new_focus = Self::first_leaf_id(&first);
                    match new_second {
                        Some(s) => Some((
                            Some(SplitNode::Split {
                                direction,
                                ratio,
                                first,
                                second: Box::new(s),
                            }),
                            removed,
                            new_focus,
                        )),
                        None => Some((Some(*first), removed, new_focus)),
                    }
                } else {
                    None
                }
            }
        }
    }

    fn first_leaf_id(node: &SplitNode<T>) -> Option<LeafId> {
        match node {
            SplitNode::Leaf { id, .. } => Some(*id),
            SplitNode::Split { first, .. } => Self::first_leaf_id(first),
        }
    }

    pub fn leaf_ids(&self) -> Vec<LeafId> {
        let mut ids = Vec::new();
        if let Some(ref node) = self.root {
            Self::collect_leaf_ids(node, &mut ids);
        }
        ids
    }

    fn collect_leaf_ids(node: &SplitNode<T>, ids: &mut Vec<LeafId>) {
        match node {
            SplitNode::Leaf { id, .. } => ids.push(*id),
            SplitNode::Split { first, second, .. } => {
                Self::collect_leaf_ids(first, ids);
                Self::collect_leaf_ids(second, ids);
            }
        }
    }

    pub fn focus_next(&mut self) -> bool {
        let ids = self.leaf_ids();
        if ids.is_empty() {
            return false;
        }

        let current_idx = self
            .focused
            .and_then(|f| ids.iter().position(|id| *id == f))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % ids.len();
        self.focused = Some(ids[next_idx]);
        true
    }

    pub fn focus_prev(&mut self) -> bool {
        let ids = self.leaf_ids();
        if ids.is_empty() {
            return false;
        }

        let current_idx = self
            .focused
            .and_then(|f| ids.iter().position(|id| *id == f))
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            ids.len() - 1
        } else {
            current_idx - 1
        };
        self.focused = Some(ids[prev_idx]);
        true
    }
}
