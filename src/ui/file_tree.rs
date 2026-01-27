//! Left sidebar file tree component.

#![allow(dead_code)]

use crate::domain::{Diff, DiffStats};
use crate::ui::styles;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding},
    Frame,
};
use std::collections::BTreeMap;

/// A node in the file tree.
#[derive(Debug, Clone)]
pub enum TreeNode {
    Directory {
        name: String,
        expanded: bool,
        children: Vec<TreeNode>,
    },
    File {
        name: String,
        path: String,
        stats: DiffStats,
        index: usize,
    },
}

impl TreeNode {
    pub fn name(&self) -> &str {
        match self {
            TreeNode::Directory { name, .. } => name,
            TreeNode::File { name, .. } => name,
        }
    }
}

/// Build a file tree from a diff.
pub fn build_tree(diff: &Diff) -> Vec<TreeNode> {
    // Group files by directory
    let mut dir_map: BTreeMap<String, Vec<(String, DiffStats, usize)>> = BTreeMap::new();

    for (idx, file) in diff.files.iter().enumerate() {
        let path = &file.path;
        let parts: Vec<&str> = path.rsplitn(2, '/').collect();

        let (filename, dir) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (parts[0].to_string(), String::new())
        };

        dir_map
            .entry(dir)
            .or_default()
            .push((filename, file.stats, idx));
    }

    // Build tree nodes
    let mut nodes = Vec::new();

    for (dir, files) in dir_map {
        if dir.is_empty() {
            // Root-level files
            for (name, stats, idx) in files {
                nodes.push(TreeNode::File {
                    name: name.clone(),
                    path: name,
                    stats,
                    index: idx,
                });
            }
        } else {
            // Directory with files
            let children: Vec<TreeNode> = files
                .into_iter()
                .map(|(name, stats, idx)| TreeNode::File {
                    name: name.clone(),
                    path: format!("{}/{}", dir, name),
                    stats,
                    index: idx,
                })
                .collect();

            nodes.push(TreeNode::Directory {
                name: dir,
                expanded: true,
                children,
            });
        }
    }

    nodes
}

/// Flatten tree for display, respecting expanded state.
pub fn flatten_tree(nodes: &[TreeNode]) -> Vec<FlatItem> {
    let mut items = Vec::new();
    flatten_recursive(nodes, 0, &mut items);
    items
}

#[derive(Debug, Clone)]
pub struct FlatItem {
    pub depth: usize,
    pub node_index: usize,
    pub is_directory: bool,
    pub is_expanded: bool,
    pub name: String,
    pub path: Option<String>,
    pub stats: Option<DiffStats>,
    pub file_index: Option<usize>,
}

fn flatten_recursive(nodes: &[TreeNode], depth: usize, items: &mut Vec<FlatItem>) {
    for (idx, node) in nodes.iter().enumerate() {
        match node {
            TreeNode::Directory {
                name,
                expanded,
                children,
            } => {
                items.push(FlatItem {
                    depth,
                    node_index: idx,
                    is_directory: true,
                    is_expanded: *expanded,
                    name: name.clone(),
                    path: None,
                    stats: None,
                    file_index: None,
                });
                if *expanded {
                    flatten_recursive(children, depth + 1, items);
                }
            }
            TreeNode::File {
                name,
                path,
                stats,
                index,
            } => {
                items.push(FlatItem {
                    depth,
                    node_index: idx,
                    is_directory: false,
                    is_expanded: false,
                    name: name.clone(),
                    path: Some(path.clone()),
                    stats: Some(*stats),
                    file_index: Some(*index),
                });
            }
        }
    }
}

/// Render the file tree sidebar.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    flat_items: &[FlatItem],
    selected: usize,
    list_state: &mut ListState,
) {
    let items: Vec<ListItem> = flat_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let indent = "  ".repeat(item.depth);
            let icon = if item.is_directory {
                if item.is_expanded { "▼ " } else { "▶ " }
            } else {
                "  "
            };

            let mut spans = vec![
                Span::raw(indent),
                Span::styled(
                    icon,
                    if item.is_directory {
                        styles::style_directory()
                    } else {
                        Style::default()
                    },
                ),
            ];

            if item.is_directory {
                spans.push(Span::styled(&item.name, styles::style_directory()));
            } else {
                spans.push(Span::styled(&item.name, styles::style_default()));

                // Add stats
                if let Some(stats) = item.stats {
                    spans.push(Span::raw(" "));
                    if stats.additions > 0 {
                        spans.push(Span::styled(
                            format!("+{}", stats.additions),
                            styles::style_stat_addition(),
                        ));
                    }
                    if stats.deletions > 0 {
                        if stats.additions > 0 {
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(
                            format!("-{}", stats.deletions),
                            styles::style_stat_deletion(),
                        ));
                    }
                }
            }

            let style = if i == selected {
                styles::style_selected()
            } else {
                Style::default()
            };

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    list_state.select(Some(selected));

    let block = Block::default()
        .title(" Files ")
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(styles::FG_MUTED))
        .style(styles::style_sidebar())
        .padding(Padding::new(1, 1, 1, 0));

    let list = List::new(items).block(block).highlight_style(styles::style_selected());

    frame.render_stateful_widget(list, area, list_state);
}
