//! Cached, virtualized views of a completed scan. Expanding and filtering never
//! touch the filesystem, and flattening uses an explicit stack for deep trees.
use crate::scanner::{Node, Scan};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Tree,
    Largest,
    Extensions,
    Overview,
    Recommendations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    Name,
    #[default]
    Size,
    Files,
    Folders,
    Modified,
    Accessed,
}

#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub index: usize,
    pub depth: usize,
}

pub struct Browser {
    pub mode: Mode,
    pub scope: usize,
    pub expanded: HashSet<usize>,
    pub sort: Sort,
    pub descending: bool,
    pub query: String,
    pub extension: Option<String>,
    pub folders_only: bool,
    sorted: HashMap<usize, Vec<usize>>,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            mode: Mode::Tree,
            scope: 0,
            expanded: HashSet::from([0]),
            sort: Sort::Size,
            descending: true,
            query: String::new(),
            extension: None,
            folders_only: false,
            sorted: HashMap::new(),
        }
    }
}

pub fn extension(node: &Node) -> String {
    Path::new(&node.name)
        .extension()
        .map(|v| v.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "no extension".into())
}

impl Browser {
    pub fn with_mode(mode: Mode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    pub fn set_sort(&mut self, sort: Sort) {
        if self.sort == sort {
            self.descending = !self.descending;
        } else {
            self.sort = sort;
            self.descending = sort != Sort::Name;
        }
        self.sorted.clear();
    }

    fn compare(&self, scan: &Scan, a: usize, b: usize) -> Ordering {
        let (left, right) = (&scan.nodes[a], &scan.nodes[b]);
        let order = match self.sort {
            Sort::Name => left.name.cmp(&right.name),
            Sort::Size => left.bytes.cmp(&right.bytes),
            Sort::Files => left.files.cmp(&right.files),
            Sort::Folders => left.folders.cmp(&right.folders),
            Sort::Modified => left.details.modified.cmp(&right.details.modified),
            Sort::Accessed => left.details.accessed.cmp(&right.details.accessed),
        };
        (if self.descending {
            order.reverse()
        } else {
            order
        })
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| a.cmp(&b))
    }

    fn cache_children(&mut self, scan: &Scan, index: usize) {
        if self.sorted.contains_key(&index) {
            return;
        }
        let mut children = scan.nodes[index].children.clone();
        children.sort_unstable_by(|a, b| self.compare(scan, *a, *b));
        self.sorted.insert(index, children);
    }

    pub fn rows(&mut self, scan: &Scan) -> Vec<Row> {
        let query = self.query.to_lowercase();
        let matches = |node: &Node| {
            (query.is_empty() || node.name.to_string_lossy().to_lowercase().contains(&query))
                && self
                    .extension
                    .as_ref()
                    .is_none_or(|ext| !node.directory && extension(node) == *ext)
        };
        if self.mode == Mode::Largest {
            let mut indices: Vec<_> = scan
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| !n.directory && matches(n))
                .map(|(i, _)| i)
                .collect();
            // Membership is always the largest 1,000 *matching* files. The chosen
            // column changes their display order, not which files make the list.
            if indices.len() > 1000 {
                indices.select_nth_unstable_by(1000, |a, b| {
                    scan.nodes[*b]
                        .bytes
                        .cmp(&scan.nodes[*a].bytes)
                        .then_with(|| a.cmp(b))
                });
                indices.truncate(1000);
            }
            indices.sort_unstable_by(|a, b| self.compare(scan, *a, *b));
            return indices
                .into_iter()
                .map(|index| Row { index, depth: 0 })
                .collect();
        }
        if matches!(
            self.mode,
            Mode::Extensions | Mode::Overview | Mode::Recommendations
        ) {
            return Vec::new();
        }
        let filtering = !query.is_empty() || self.extension.is_some();
        let mut visible = Vec::new();
        if filtering {
            visible = scan
                .nodes
                .iter()
                .map(|n| (!self.folders_only || n.directory) && matches(n))
                .collect();
            for i in (1..visible.len()).rev() {
                if visible[i]
                    && let Some(parent) = scan.nodes[i].parent
                {
                    visible[parent] = true;
                }
            }
            if !visible[self.scope] {
                return Vec::new();
            }
        }
        let mut output = Vec::new();
        let mut stack = vec![(self.scope, 0)];
        while let Some((index, depth)) = stack.pop() {
            let node = &scan.nodes[index];
            if (self.folders_only && !node.directory) || (filtering && !visible[index]) {
                continue;
            }
            output.push(Row { index, depth });
            if node.directory && (filtering || self.expanded.contains(&index)) {
                self.cache_children(scan, index);
                for child in self.sorted[&index].iter().rev() {
                    stack.push((*child, depth + 1));
                }
            }
        }
        output
    }

    pub fn expand_to(&mut self, scan: &Scan, levels: usize) {
        self.expanded.clear();
        let mut stack = vec![(self.scope, 0)];
        while let Some((index, depth)) = stack.pop() {
            if depth >= levels || !scan.nodes[index].directory {
                continue;
            }
            self.expanded.insert(index);
            for child in &scan.nodes[index].children {
                stack.push((*child, depth + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        platform::FileDetails,
        scanner::{Progress, Scan},
    };
    fn fixture() -> Scan {
        let node = |name: &str, parent, children, directory, bytes| Node {
            name: name.into(),
            parent,
            children,
            directory,
            bytes,
            files: u64::from(!directory),
            folders: 0,
            details: FileDetails::default(),
            incomplete: false,
        };
        Scan {
            root: "root".into(),
            nodes: vec![
                node("root", None, vec![1, 2], true, 18),
                node("nested", Some(0), vec![3, 4], true, 13),
                node("outside.bin", Some(0), vec![], false, 5),
                node("find-me.txt", Some(1), vec![], false, 10),
                node("small.bin", Some(1), vec![], false, 3),
            ],
            progress: Progress::default(),
            cancelled: false,
            issues: vec![],
            extensions: HashMap::new(),
            workers: 1,
        }
    }
    #[test]
    fn expanded_rows_preserve_tree_and_sibling_order() {
        let scan = fixture();
        let mut b = Browser::default();
        assert_eq!(
            b.rows(&scan).iter().map(|r| r.index).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        b.expanded.insert(1);
        let rows = b.rows(&scan);
        assert_eq!(
            rows.iter().map(|r| (r.index, r.depth)).collect::<Vec<_>>(),
            [(0, 0), (1, 1), (3, 2), (4, 2), (2, 1)]
        );
        b.set_sort(Sort::Size); // ascending, including after children were cached
        assert_eq!(
            b.rows(&scan).iter().map(|r| r.index).collect::<Vec<_>>(),
            [0, 2, 1, 4, 3]
        );
    }
    #[test]
    fn deep_filter_keeps_ancestors_without_changing_expansion() {
        let scan = fixture();
        let mut b = Browser {
            query: "FIND-ME".into(),
            ..Default::default()
        };
        assert_eq!(
            b.rows(&scan).iter().map(|r| r.index).collect::<Vec<_>>(),
            [0, 1, 3]
        );
        b.query.clear();
        assert_eq!(b.rows(&scan).len(), 3);
        b.query = "missing".into();
        assert!(b.rows(&scan).is_empty());
    }
    #[test]
    fn extension_filter_and_scope_and_folder_view() {
        let scan = fixture();
        let mut b = Browser {
            mode: Mode::Largest,
            extension: Some("bin".into()),
            ..Default::default()
        };
        assert_eq!(
            b.rows(&scan).iter().map(|r| r.index).collect::<Vec<_>>(),
            [2, 4]
        );
        b.mode = Mode::Tree;
        b.extension = None;
        b.scope = 1;
        b.expand_to(&scan, 1);
        assert_eq!(b.rows(&scan).len(), 3);
        b.folders_only = true;
        assert_eq!(b.rows(&scan).len(), 1);
    }
}
