//! Left sidebar file tree component with filter support.

#![allow(dead_code)]

use crate::domain::{Diff, DiffStats};
use crate::ui::styles;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};

/// Represents the type of change for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

impl ChangeType {
    pub fn from_stats(stats: DiffStats) -> Self {
        if stats.deletions == 0 && stats.additions > 0 {
            ChangeType::Added
        } else if stats.additions == 0 && stats.deletions > 0 {
            ChangeType::Deleted
        } else {
            ChangeType::Modified
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ChangeType::Added => "●",
            ChangeType::Modified => "●",
            ChangeType::Deleted => "●",
        }
    }

    pub fn style(&self) -> Style {
        match self {
            ChangeType::Added => Style::default().fg(styles::FG_ADDITION),
            ChangeType::Modified => Style::default().fg(styles::FG_HUNK),
            ChangeType::Deleted => Style::default().fg(styles::FG_DELETION),
        }
    }
}

/// A node in the hierarchical file tree.
#[derive(Debug, Clone)]
pub enum TreeNode {
    Directory {
        name: String,
        path: String,
        expanded: bool,
        children: Vec<TreeNode>,
    },
    File {
        name: String,
        path: String,
        stats: DiffStats,
        change_type: ChangeType,
        file_index: usize,
    },
}

impl TreeNode {
    pub fn name(&self) -> &str {
        match self {
            TreeNode::Directory { name, .. } => name,
            TreeNode::File { name, .. } => name,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            TreeNode::Directory { path, .. } => path,
            TreeNode::File { path, .. } => path,
        }
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, TreeNode::Directory { .. })
    }
}

/// Build a hierarchical tree from the diff.
pub fn build_tree(diff: &Diff) -> Vec<TreeNode> {
    // Group files by directory path
    let mut root_children: Vec<TreeNode> = Vec::new();

    for (idx, file) in diff.files.iter().enumerate() {
        let parts: Vec<&str> = file.path.split('/').collect();
        insert_into_tree(&mut root_children, &parts, file.stats, idx);
    }

    // Sort: directories first, then alphabetically
    sort_tree(&mut root_children);
    root_children
}

fn insert_into_tree(
    nodes: &mut Vec<TreeNode>,
    parts: &[&str],
    stats: DiffStats,
    file_index: usize,
) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // This is a file
        let name = parts[0].to_string();
        nodes.push(TreeNode::File {
            name: name.clone(),
            path: name,
            stats,
            change_type: ChangeType::from_stats(stats),
            file_index,
        });
        return;
    }

    // This is a directory path
    let dir_name = parts[0];
    let remaining = &parts[1..];

    // Find or create directory
    let dir_idx = nodes.iter().position(|n| {
        matches!(n, TreeNode::Directory { name, .. } if name == dir_name)
    });

    match dir_idx {
        Some(idx) => {
            if let TreeNode::Directory { children, .. } = &mut nodes[idx] {
                insert_into_tree(children, remaining, stats, file_index);
            }
        }
        None => {
            let mut children = Vec::new();
            insert_into_tree(&mut children, remaining, stats, file_index);
            nodes.push(TreeNode::Directory {
                name: dir_name.to_string(),
                path: dir_name.to_string(),
                expanded: true,
                children,
            });
        }
    }
}

fn sort_tree(nodes: &mut Vec<TreeNode>) {
    nodes.sort_by(|a, b| {
        match (a.is_directory(), b.is_directory()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name().cmp(b.name()),
        }
    });

    for node in nodes.iter_mut() {
        if let TreeNode::Directory { children, .. } = node {
            sort_tree(children);
        }
    }
}

/// Flattened item for display.
#[derive(Debug, Clone)]
pub struct FlatItem {
    pub depth: usize,
    pub is_directory: bool,
    pub is_expanded: bool,
    pub name: String,
    pub full_path: String,
    pub stats: Option<DiffStats>,
    pub change_type: Option<ChangeType>,
    pub file_index: Option<usize>,
    pub tree_path: Vec<usize>, // Path to this node in the tree
}

/// Flatten the tree for display, respecting expanded state and filter.
pub fn flatten_tree(nodes: &[TreeNode], filter: &str) -> Vec<FlatItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();
    flatten_recursive(nodes, 0, &mut items, &filter_lower, &mut vec![], "");
    items
}

fn flatten_recursive(
    nodes: &[TreeNode],
    depth: usize,
    items: &mut Vec<FlatItem>,
    filter: &str,
    tree_path: &mut Vec<usize>,
    parent_path: &str,
) {
    for (idx, node) in nodes.iter().enumerate() {
        tree_path.push(idx);

        match node {
            TreeNode::Directory {
                name,
                expanded,
                children,
                ..
            } => {
                let full_path = if parent_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", parent_path, name)
                };

                // Check if any children match the filter
                let has_matching_children = filter.is_empty() || directory_has_matching_files(children, filter);

                if has_matching_children {
                    items.push(FlatItem {
                        depth,
                        is_directory: true,
                        is_expanded: *expanded,
                        name: name.clone(),
                        full_path: full_path.clone(),
                        stats: None,
                        change_type: None,
                        file_index: None,
                        tree_path: tree_path.clone(),
                    });

                    if *expanded {
                        flatten_recursive(children, depth + 1, items, filter, tree_path, &full_path);
                    }
                }
            }
            TreeNode::File {
                name,
                stats,
                change_type,
                file_index,
                ..
            } => {
                let full_path = if parent_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", parent_path, name)
                };

                // Apply filter
                if filter.is_empty() || full_path.to_lowercase().contains(filter) {
                    items.push(FlatItem {
                        depth,
                        is_directory: false,
                        is_expanded: false,
                        name: name.clone(),
                        full_path,
                        stats: Some(*stats),
                        change_type: Some(*change_type),
                        file_index: Some(*file_index),
                        tree_path: tree_path.clone(),
                    });
                }
            }
        }

        tree_path.pop();
    }
}

fn directory_has_matching_files(nodes: &[TreeNode], filter: &str) -> bool {
    for node in nodes {
        match node {
            TreeNode::Directory { children, .. } => {
                if directory_has_matching_files(children, filter) {
                    return true;
                }
            }
            TreeNode::File { name, .. } => {
                if name.to_lowercase().contains(filter) {
                    return true;
                }
            }
        }
    }
    false
}

/// Toggle expand/collapse for a directory at the given tree path.
pub fn toggle_directory(nodes: &mut [TreeNode], tree_path: &[usize]) {
    if tree_path.is_empty() {
        return;
    }

    let idx = tree_path[0];
    if idx >= nodes.len() {
        return;
    }

    if tree_path.len() == 1 {
        if let TreeNode::Directory { expanded, .. } = &mut nodes[idx] {
            *expanded = !*expanded;
        }
    } else {
        if let TreeNode::Directory { children, .. } = &mut nodes[idx] {
            toggle_directory(children, &tree_path[1..]);
        }
    }
}

/// Render the file tree sidebar with filter input.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    flat_items: &[FlatItem],
    selected: usize,
    current_file: usize,
    viewed: &std::collections::HashSet<usize>,
    filter: &str,
    filter_focused: bool,
    list_state: &mut ListState,
) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    // Filter input with search icon
    let filter_border_color = if filter_focused {
        styles::FG_PATH
    } else {
        styles::FG_BORDER
    };

    let filter_text = if filter.is_empty() && !filter_focused {
        " Filter files...".to_string()
    } else {
        format!(" {}", filter)
    };

    let filter_style = if filter_focused {
        Style::default().fg(styles::FG_DEFAULT)
    } else {
        Style::default().fg(styles::FG_MUTED)
    };

    let filter_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(filter_border_color))
        .title(Span::styled(" Files ", Style::default().fg(styles::FG_DEFAULT)));

    let filter_input = Paragraph::new(filter_text)
        .style(filter_style)
        .block(filter_block);
    frame.render_widget(filter_input, chunks[0]);

    // File tree
    let items: Vec<ListItem> = flat_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let indent = "  ".repeat(item.depth);
            let is_selected = i == selected;
            let is_current = item.file_index == Some(current_file);
            let is_viewed = item.file_index.map(|idx| viewed.contains(&idx)).unwrap_or(false);

            let mut spans = vec![Span::raw(indent)];

            if item.is_directory {
                // Directory with expand/collapse icon
                let icon = if item.is_expanded { "▾ " } else { "▸ " };
                spans.push(Span::styled(icon, Style::default().fg(styles::FG_MUTED)));
                spans.push(Span::styled(
                    format!(" {}", item.name),
                    Style::default().fg(styles::FG_DIRECTORY).add_modifier(Modifier::BOLD),
                ));
            } else {
                // File with viewed checkbox and change type dot
                let checkbox = if is_viewed { "☑ " } else { "☐ " };
                let checkbox_style = if is_viewed {
                    Style::default().fg(styles::FG_ADDITION)
                } else {
                    Style::default().fg(styles::FG_MUTED)
                };
                spans.push(Span::styled(checkbox, checkbox_style));

                if let Some(change_type) = item.change_type {
                    spans.push(Span::styled(change_type.icon(), change_type.style()));
                    spans.push(Span::raw(" "));
                }

                let file_style = if is_viewed {
                    Style::default().fg(styles::FG_MUTED) // Dimmed when viewed
                } else if is_current {
                    Style::default().fg(styles::FG_DEFAULT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(styles::FG_DEFAULT)
                };
                spans.push(Span::styled(&item.name, file_style));

                // Show stats for files
                if let Some(stats) = &item.stats {
                    spans.push(Span::styled(
                        format!(" +{} -{}", stats.additions, stats.deletions),
                        Style::default().fg(styles::FG_MUTED),
                    ));
                }
            }

            let style = if is_selected {
                Style::default().bg(styles::BG_SELECTED)
            } else if is_current && !item.is_directory {
                Style::default().bg(styles::BG_HOVER)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    list_state.select(Some(selected));

    let tree_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(styles::FG_BORDER))
        .padding(Padding::new(1, 0, 0, 0));

    let list = List::new(items)
        .block(tree_block)
        .highlight_style(Style::default().bg(styles::BG_SELECTED));

    frame.render_stateful_widget(list, chunks[1], list_state);
}
