//! The folder tree: drives, subfolders and one row of it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use eframe::egui::{self, Align2, FontFamily, FontId, Pos2, Rect, Sense, Vec2};

use crate::theme::{self, icon, paint_fit, palette, space, text};
use crate::ui::ROW;

/// Indentation per level.
pub(crate) const INDENT: f32 = 12.0;

pub(crate) struct TreeItem {
    pub(crate) path: PathBuf,
    pub(crate) depth: usize,
    pub(crate) expanded: bool,
}

/// Drives, expanded folders and the subfolders read for them. The visible rows
/// are rebuilt only when something about them changes – not on every frame.
pub(crate) struct Tree {
    roots: Vec<PathBuf>,
    expanded: HashSet<PathBuf>,
    /// Subfolders read per folder; read again when expanded.
    children: HashMap<PathBuf, Vec<PathBuf>>,
    /// The visible rows; `None` when they have to be rebuilt.
    items: Option<Rc<[TreeItem]>>,
}

impl Tree {
    /// At startup, C: is expanded on Windows, otherwise `/`.
    pub(crate) fn new(roots: Vec<PathBuf>) -> Self {
        let mut expanded = HashSet::new();
        if let Some(first) = start_root(&roots) {
            expanded.insert(first);
        }
        Self {
            roots,
            expanded,
            children: HashMap::new(),
            items: None,
        }
    }

    /// New list of drives; the rows are rebuilt only if it has changed.
    pub(crate) fn set_roots(&mut self, roots: Vec<PathBuf>) {
        if roots != self.roots {
            self.roots = roots;
            self.items = None;
        }
    }

    pub(crate) fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Expands a folder. `reread` reads its subfolders again, like a click on it.
    pub(crate) fn expand(&mut self, path: &Path, reread: bool) {
        if reread {
            self.children.remove(path);
        }
        self.expanded.insert(path.to_path_buf());
        self.items = None;
    }

    pub(crate) fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
        self.items = None;
    }

    /// The subfolders, read on first use.
    pub(crate) fn children(&mut self, path: &Path) -> &[PathBuf] {
        self.children
            .entry(path.to_path_buf())
            .or_insert_with(|| read_child_dirs(path))
    }

    /// The visible rows, top to bottom. An `Rc`, so the tree can be modified
    /// while they are being iterated.
    pub(crate) fn items(&mut self) -> Rc<[TreeItem]> {
        if let Some(items) = &self.items {
            return Rc::clone(items);
        }
        let mut items = Vec::new();
        for root in self.roots.clone() {
            self.push(&root, 0, &mut items);
        }
        let items: Rc<[TreeItem]> = items.into();
        self.items = Some(Rc::clone(&items));
        items
    }

    fn push(&mut self, path: &Path, depth: usize, items: &mut Vec<TreeItem>) {
        let expanded = self.expanded.contains(path);
        items.push(TreeItem {
            path: path.to_path_buf(),
            depth,
            expanded,
        });
        if expanded {
            for child in self.children(path).to_vec() {
                self.push(&child, depth + 1, items);
            }
        }
    }
}

/// Returns the row's response and the rectangle of the arrow.
pub(crate) fn paint_tree_row(
    ui: &mut egui::Ui,
    item: &TreeItem,
    selected: bool,
    focused: bool,
    width: f32,
) -> (egui::Response, Rect) {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::CLICK);
    theme::row_ground(ui, rect, selected, focused, response.hovered());
    let y = rect.center().y;
    let x = rect.left() + space::S1 + item.depth as f32 * INDENT;
    // Click target of the arrow at least 24 wide (§4.2, WCAG 2.2).
    let chevron = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(theme::COMPACT, ROW));
    let glyph = if item.expanded {
        icon::CHEVRON_DOWN
    } else {
        icon::CHEVRON_RIGHT
    };
    let icon_font = FontId::new(theme::ICON, FontFamily::Proportional);
    ui.painter().text(
        chevron.center(),
        Align2::CENTER_CENTER,
        glyph.to_string(),
        icon_font.clone(),
        p.muted,
    );
    let folder_x = chevron.right();
    ui.painter().text(
        Pos2::new(folder_x, y),
        Align2::LEFT_CENTER,
        icon::FOLDER.to_string(),
        icon_font,
        if selected { p.accent_strong } else { p.muted },
    );
    let label = tree_label(&item.path);
    let label_x = folder_x + theme::ICON + space::S2;
    let truncated = paint_fit(
        ui,
        Pos2::new(label_x, y),
        &label,
        (rect.right() - label_x - space::S2).max(8.0),
        FontId::new(text::BODY, FontFamily::Proportional),
        p.text,
        false,
    );
    let response = if truncated {
        response.on_hover_text(label)
    } else {
        response
    };
    (response, chevron)
}

/// Drives from the bitmask of `GetLogicalDrives`. The call touches no drive,
/// so a disconnected network drive does not slow down the UI.
#[cfg(windows)]
#[allow(unsafe_code)] // Windows API; every block has a // SAFETY: comment
pub(crate) fn filesystem_roots() -> Vec<PathBuf> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLogicalDrives() -> u32;
    }
    // SAFETY: GetLogicalDrives takes no parameters and only returns a bitmask.
    let mask = unsafe { GetLogicalDrives() };
    let mut roots: Vec<PathBuf> = (0..26u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| PathBuf::from(format!("{}:\\", char::from(b'A' + bit))))
        .collect();
    if roots.is_empty() {
        roots.push(PathBuf::from("C:\\"));
    }
    roots
}

#[cfg(not(windows))]
pub(crate) fn filesystem_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

/// Expanded at startup: C: on Windows, otherwise `/`.
pub(crate) fn start_root(roots: &[PathBuf]) -> Option<PathBuf> {
    roots
        .iter()
        .find(|root| root.as_os_str().eq_ignore_ascii_case("C:\\"))
        .or_else(|| roots.first())
        .cloned()
}

pub(crate) fn parent_chain(path: &Path) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    let mut current = PathBuf::new();
    let comps: Vec<_> = path.components().collect();
    if comps.is_empty() {
        return chain;
    }
    for comp in &comps[..comps.len() - 1] {
        current.push(comp);
        chain.push(current.clone());
    }
    chain
}

pub(crate) fn tree_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn read_child_dirs(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Ok(read) = std::fs::read_dir(path) else {
        return dirs;
    };
    for entry in read.flatten() {
        let child = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if dir_visible(&child, kind) {
            dirs.push(child);
        }
    }
    // Ignoring case, with case as the tie-breaker; each name read once.
    dirs.sort_by_cached_key(|dir| {
        let label = tree_label(dir);
        (label.to_lowercase(), label)
    });
    dirs
}

pub(crate) fn dir_visible(path: &Path, kind: std::fs::FileType) -> bool {
    if kind.is_dir() {
        return true;
    }
    if !kind.is_symlink() {
        return false;
    }
    let Ok(canon) = path.canonicalize() else {
        return false;
    };
    if !canon.is_dir() {
        return false;
    }
    let mut parent = path.parent();
    while let Some(current) = parent {
        if current.as_os_str().is_empty() {
            break;
        }
        if current.canonicalize().ok().as_ref() == Some(&canon) {
            return false;
        }
        parent = current.parent();
    }
    true
}
