//! The window's state: the folder shown, its items, the selection, history and search.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Instant, UNIX_EPOCH};

use rune::{Latest, PointerPress, TextEdit, Tx, Waker};
use rune_kit::shell::{PaneState, Reorder};
use rune_kit::SortDir;

use crate::format::{self, Look};

#[derive(Clone, Debug)]
pub struct Entry {
    /// The row's id: the name in a folder listing, the full path in search results. Either way
    /// `dir.join(id)` is the item's path.
    pub id: String,
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub modified: i64,
    pub created: i64,
    pub kind: String,
    pub look: Look,
    /// Finder tags, by name.
    pub tags: Vec<String>,
}

impl Entry {
    pub fn read(path: &Path, id: String) -> Option<Entry> {
        let lm = std::fs::symlink_metadata(path).ok()?;
        // `fs::metadata` follows symlinks, so a link to a folder opens as one.
        let m = std::fs::metadata(path).unwrap_or(lm.clone());
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let secs = |t: std::io::Result<std::time::SystemTime>| t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
        let (dir, link) = (m.is_dir(), lm.file_type().is_symlink());
        Some(Entry {
            tags: crate::mac::tags(path),
            id,
            kind: format::kind(&name, dir, link),
            look: format::look(&name, dir),
            size: m.len(),
            modified: secs(m.modified()),
            created: secs(m.created()),
            name,
            dir,
        })
    }
    /// A bundle (.app) is a folder Finder launches instead of entering.
    pub fn is_folder(&self) -> bool {
        self.dir && self.look == Look::Folder
    }
    /// How deep the row sits in the list's outline: 0 in the folder itself, 1 inside an
    /// expanded folder, and so on. Search results (absolute ids) are all at the top.
    pub fn depth(&self) -> usize {
        if self.id.starts_with('/') { 0 } else { self.id.matches('/').count() }
    }
    /// The id of the expanded folder this row sits in, if any.
    pub fn parent(&self) -> Option<&str> {
        if self.id.starts_with('/') { None } else { self.id.rsplit_once('/').map(|(p, _)| p) }
    }
}

/// The items of `dir`, with ids under `prefix` (the folder's own id) when it's an expanded
/// folder of the listing.
pub fn read_entries(dir: &Path, prefix: Option<&str>) -> std::io::Result<Vec<Entry>> {
    let id = |name: String| prefix.map_or_else(|| name.clone(), |p| format!("{p}/{name}"));
    Ok(std::fs::read_dir(dir)?.flatten().filter_map(|e| Entry::read(&e.path(), id(e.file_name().to_string_lossy().into_owned()))).collect())
}

/// How the list groups its rows, from the toolbar's group menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    None,
    Kind,
    Modified,
    Size,
}

/// The toolbar menu that's open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    Group,
    Share,
    Tags,
    More,
}

/// Folder sizes being counted off the UI thread, one folder at a time.
pub struct Sizer {
    pub rx: Receiver<(PathBuf, u64)>,
    pub stop: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Icons,
    List,
    Columns,
    Gallery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Spotlight's whole Mac, Finder's default.
    Mac,
    /// The folder the search started in, and everything inside it.
    Folder,
}

/// The selection, in every view: ids in the order picked, the keyboard cursor, and where a
/// Shift-click extends from.
#[derive(Default)]
pub struct Picks {
    pub set: Vec<String>,
    pub cursor: Option<String>,
    pub anchor: Option<String>,
}

/// Get Info's window: one item, with its folder size filled in once it's counted.
/// One column of the column view: a folder's items and the one picked in it.
pub struct Column {
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub pick: Option<String>,
}

impl Column {
    pub fn read(dir: PathBuf, hidden: bool) -> Column {
        let mut entries: Vec<Entry> = std::fs::read_dir(&dir)
            .map(|d| d.flatten().filter_map(|e| Entry::read(&e.path(), e.file_name().to_string_lossy().into_owned())).collect())
            .unwrap_or_default();
        entries.retain(|e| hidden || !e.name.starts_with('.'));
        entries.sort_by(|a, b| rune_kit::natural_cmp(&a.name, &b.name));
        Column { dir, entries, pick: None }
    }
}

pub struct Info {
    pub path: PathBuf,
    pub entry: Entry,
    /// Bytes and items, counted off the UI thread for a folder.
    pub total: Option<(u64, u64)>,
}

pub struct App {
    pub sidebar: PaneState,
    pub dir: PathBuf,
    pub back: Vec<PathBuf>,
    pub fwd: Vec<PathBuf>,
    pub all: Vec<Entry>,
    /// Indices into `all` after hidden files are dropped, in sort order.
    pub rows: Vec<usize>,
    pub view: View,
    /// The list's sort: a column id and direction.
    pub sort: (String, SortDir),
    pub picks: Picks,
    /// The column view's columns, from the window's folder rightwards.
    pub cols: Vec<Column>,
    /// Whether ⌥ is held, which changes the context menu's items as in Finder.
    pub alt: bool,
    /// Items ⌥⌘⌫ is asking to delete for good.
    pub confirm_delete: Option<Vec<PathBuf>>,
    /// The Quick Look window Space opened, which Space closes again.
    pub quick_look: Option<std::process::Child>,
    /// System file icons and image previews, decoded off the UI thread (after `setup`).
    pub images: Option<rune::ImageCache>,
    pub search: TextEdit,
    pub scope: Scope,
    /// Search results, found off the UI thread.
    pub found: Latest<Vec<Entry>>,
    pub hidden: bool,
    pub menu: Option<PointerPress>,
    pub renaming: Option<String>,
    pub rename: TextEdit,
    /// What ⌘C copied, for when the system clipboard can't be read back.
    pub clip: Vec<PathBuf>,
    pub volumes: Vec<PathBuf>,
    pub free: Option<u64>,
    pub error: Option<String>,
    pub info: Option<Info>,
    pub totals: Latest<(PathBuf, u64, u64)>,
    /// "Go to Folder" (⇧⌘G), while open.
    pub goto: Option<TextEdit>,
    /// Type-to-select: what's been typed and when the last key came.
    pub typed: (String, Option<Instant>),
    pub drop_over: Option<String>,
    /// Folders open in the list's outline, by id.
    pub expanded: Vec<String>,
    pub group: Group,
    /// Where each group starts in `rows`, and its title.
    pub groups: Vec<(usize, String)>,
    pub popup: Option<Popup>,
    /// View options' "모든 크기 계산": the list shows folders' sizes.
    pub calc_sizes: bool,
    /// Folder sizes counted so far, by path.
    pub sizes: HashMap<PathBuf, u64>,
    pub sizer: Option<Sizer>,
    pub waker: Option<Waker>,
    /// The sidebar's 즐겨찾기, and the file they're kept in (None keeps them in memory only).
    pub favorites: Vec<PathBuf>,
    pub favorites_file: Option<PathBuf>,
    /// A sidebar favorite's context menu: where it opened and for which folder.
    pub side_menu: Option<(PointerPress, PathBuf)>,
    /// A favorite being dragged to a new place in the sidebar.
    pub fav_reorder: Reorder,
    /// Set by a favorite's drop, so the press that ends the drag doesn't open it.
    pub fav_moved: bool,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

pub fn init() -> App {
    let args = rune::extra_args();
    let start = args.iter().map(PathBuf::from).find(|p| p.is_dir()).unwrap_or_else(home);
    let volumes = std::fs::read_dir("/Volumes").map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    let mut s = App {
        sidebar: PaneState::new(190.0).range(140.0, 400.0),
        dir: start,
        back: vec![],
        fwd: vec![],
        all: vec![],
        rows: vec![],
        view: View::List,
        sort: ("name".into(), SortDir::Ascending),
        picks: Picks::default(),
        cols: vec![],
        alt: false,
        confirm_delete: None,
        quick_look: None,
        images: None,
        search: TextEdit::new(""),
        scope: Scope::Mac,
        found: Latest::default().debounce(std::time::Duration::from_millis(150)),
        hidden: false,
        menu: None,
        renaming: None,
        rename: TextEdit::new(""),
        clip: vec![],
        volumes,
        free: None,
        error: None,
        info: None,
        totals: Latest::default(),
        goto: None,
        typed: (String::new(), None),
        drop_over: None,
        expanded: vec![],
        group: Group::None,
        groups: vec![],
        popup: None,
        calc_sizes: false,
        sizes: HashMap::new(),
        sizer: None,
        waker: None,
        favorites: vec![],
        favorites_file: favorites_file(),
        side_menu: None,
        fav_reorder: Reorder::new(),
        fav_moved: false,
    };
    s.favorites = load_favorites(s.favorites_file.as_deref());
    load(&mut s);
    s
}

// --- listing ----------------------------------------------------------------

pub fn searching(s: &App) -> bool {
    !s.search.text.trim().is_empty()
}

// shortcut: reads the folder on the UI thread (about 20 ms per 10,000 entries on an SSD);
// move to `rune::Latest` if network volumes or huge folders stall the window.
pub fn load(s: &mut App) {
    s.error = None;
    s.free = free_space(&s.dir);
    if searching(s) {
        request_search(s);
        return;
    }
    s.all = match read_entries(&s.dir, None) {
        Ok(all) => all,
        Err(e) => {
            s.error = Some(format!("폴더를 열 수 없음: {e}"));
            vec![]
        }
    };
    // Expanded folders' items, outer folders first so their own expanded folders are found.
    s.expanded.sort_by_key(|id| id.matches('/').count());
    for id in std::mem::take(&mut s.expanded) {
        if s.all.iter().any(|e| e.id == id && e.is_folder()) {
            s.all.extend(read_entries(&s.dir.join(&id), Some(&id)).unwrap_or_default());
            s.expanded.push(id);
        }
    }
    refresh(s);
    request_sizes(s);
}

/// The list's ▸ on a folder: shows its items under it, or hides them again.
pub fn toggle_expanded(s: &mut App, id: &str) {
    if s.expanded.iter().any(|e| e == id) {
        let inside = format!("{id}/");
        s.expanded.retain(|e| e != id && !e.starts_with(&inside));
        s.all.retain(|e| !e.id.starts_with(&inside));
        refresh(s);
    } else if s.all.iter().any(|e| e.id == id && e.is_folder()) {
        s.all.extend(read_entries(&s.dir.join(id), Some(id)).unwrap_or_default());
        s.expanded.push(id.to_owned());
        refresh(s);
        request_sizes(s);
    }
}

/// → and ← in the list: open or close the focused folder (⌥ for everything inside it too);
/// ← on an item inside an open folder moves to that folder, as in Finder.
pub fn disclose_focused(s: &mut App, open: bool, all: bool, tx: &mut Tx) -> bool {
    let Some(id) = focus_id(s) else { return false };
    let Some(e) = s.all.iter().find(|e| e.id == id).cloned() else { return false };
    if open {
        if e.is_folder() && !is_expanded(s, &id) {
            toggle_expanded(s, &id);
        }
        if all {
            // Finder's ⌥→ opens every folder inside, however deep; the depth bound keeps a
            // runaway tree in check.
            let mut i = 0;
            while i < s.all.len() {
                let f = s.all[i].clone();
                if f.id.starts_with(&format!("{id}/")) && f.is_folder() && f.depth() < 8 && !is_expanded(s, &f.id) {
                    toggle_expanded(s, &f.id);
                }
                i += 1;
            }
        }
    } else if is_expanded(s, &id) {
        toggle_expanded(s, &id);
    } else if let Some(p) = e.parent().map(str::to_owned) {
        reveal(s, p, tx);
    }
    true
}

pub fn is_expanded(s: &App, id: &str) -> bool {
    s.expanded.iter().any(|e| e == id)
}

/// Whether the view shows groups: the list and icon views, as Finder's do.
pub fn groups_shown(s: &App) -> bool {
    matches!(s.view, View::List | View::Icons)
}

/// Whether the list shows its outline: only the list view, and not search results.
pub fn outline(s: &App) -> bool {
    s.view == View::List && !searching(s)
}

/// An item's size as the list shows and sorts it: a file's own, a folder's once counted with
/// "모든 크기 계산" on, else none ("--").
pub fn size_of(s: &App, e: &Entry) -> Option<u64> {
    if !e.dir {
        Some(e.size)
    } else if s.calc_sizes {
        s.sizes.get(&s.dir.join(&e.id)).copied()
    } else {
        None
    }
}

/// The group a row falls in: its order among the groups, and its title.
pub fn group_of(s: &App, e: &Entry, now: i64) -> (i64, String) {
    match s.group {
        Group::None => (0, String::new()),
        Group::Kind => (0, e.kind.clone()),
        Group::Modified => format::day_group(e.modified, now),
        Group::Size => match size_of(s, e) {
            None => (5, "--".into()),
            Some(b) if b < 1_000_000 => (4, "1MB 미만".into()),
            Some(b) if b < 100_000_000 => (3, "1MB~100MB".into()),
            Some(b) if b < 1_000_000_000 => (2, "100MB~1GB".into()),
            Some(_) => (1, "1GB 이상".into()),
        },
    }
}

/// Rebuilds `rows` from `all`: hidden files out, then the groups and the sort, with each
/// expanded folder's items under it in the list.
pub fn refresh(s: &mut App) {
    let all = &s.all;
    let shown = |i: &usize| s.hidden || !all[*i].name.starts_with('.');
    let (column, dir) = (s.sort.0.as_str(), s.sort.1);
    let order = |&a: &usize, &b: &usize| {
        let (a, b) = (&all[a], &all[b]);
        let name = || rune_kit::natural_cmp(&a.name, &b.name);
        dir.apply(match column {
            "modified" => a.modified.cmp(&b.modified).then_with(name),
            "size" => size_of(s, a).cmp(&size_of(s, b)).then_with(name),
            "kind" => a.kind.cmp(&b.kind).then_with(name),
            _ => name(),
        })
    };
    let outline = outline(s);
    let grouped = s.group != Group::None && groups_shown(s);
    let now = format::now();
    let mut top: Vec<(i64, String, usize)> = (0..all.len())
        .filter(|i| shown(i) && all[*i].depth() == 0)
        .map(|i| {
            let (rank, title) = if grouped { group_of(s, &all[i], now) } else { (0, String::new()) };
            (rank, title, i)
        })
        .collect();
    top.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| order(&a.2, &b.2)));
    let mut kids: HashMap<&str, Vec<usize>> = HashMap::new();
    if outline {
        for i in (0..all.len()).filter(shown) {
            if let Some(p) = all[i].parent() {
                kids.entry(p).or_default().push(i);
            }
        }
        for v in kids.values_mut() {
            v.sort_by(order);
        }
    }
    fn under(i: usize, all: &[Entry], kids: &HashMap<&str, Vec<usize>>, out: &mut Vec<usize>) {
        out.push(i);
        for &k in kids.get(all[i].id.as_str()).into_iter().flatten() {
            under(k, all, kids, out);
        }
    }
    let (mut rows, mut groups) = (vec![], vec![]);
    for (_, title, i) in top {
        if grouped && groups.last().is_none_or(|(_, t): &(usize, String)| *t != title) {
            groups.push((rows.len(), title));
        }
        under(i, all, &kids, &mut rows);
    }
    s.rows = rows;
    s.groups = groups;
    // Drop selected ids that left the listing (the column view's live in its columns).
    if s.view == View::Columns && !searching(s) {
        reload_cols(s);
    } else {
        s.picks.set.retain(|id| s.rows.iter().any(|&i| all[i].id == *id));
    }
}

fn free_space(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c` is a valid C string and `st` is plain data `statvfs` fills.
    let st = unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return None;
        }
        st
    };
    Some(st.f_bavail as u64 * st.f_frsize as u64)
}

// --- search -----------------------------------------------------------------

const MAX_RESULTS: usize = 2000;

pub fn request_search(s: &mut App) {
    let q = s.search.text.trim().to_owned();
    if q.is_empty() {
        s.found.cancel();
        load(s);
        return;
    }
    let root = match s.scope {
        Scope::Mac => None,
        Scope::Folder => Some(s.dir.clone()),
    };
    let hidden = s.hidden;
    s.found.request((q, root), move |(q, root): (String, Option<PathBuf>)| find(&q, root.as_deref(), hidden));
}

/// Items whose names contain `q`, as Finder's search field finds them: Spotlight on a Mac
/// (`root` limits it to one folder), else a walk of `root` (or the home folder).
pub fn find(q: &str, root: Option<&Path>, hidden: bool) -> Vec<Entry> {
    let mut paths = spotlight(q, root).unwrap_or_else(|| walk(q, root.map_or_else(home, Path::to_path_buf), hidden));
    paths.truncate(MAX_RESULTS);
    paths.into_iter().filter_map(|p| Entry::read(&p, p.to_string_lossy().into_owned())).collect()
}

fn spotlight(q: &str, root: Option<&Path>) -> Option<Vec<PathBuf>> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let mut cmd = std::process::Command::new("/usr/bin/mdfind");
    if let Some(r) = root {
        cmd.arg("-onlyin").arg(r);
    }
    let out = cmd.arg("-name").arg(q).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).lines().take(MAX_RESULTS).map(PathBuf::from).collect())
}

fn walk(q: &str, root: PathBuf, hidden: bool) -> Vec<PathBuf> {
    let q = q.to_lowercase();
    let (mut out, mut stack, mut seen) = (vec![], vec![root], 0usize);
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > 200_000 || out.len() >= MAX_RESULTS {
                return out;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if !hidden && name.starts_with('.') {
                continue;
            }
            let p = e.path();
            if name.contains(&q) {
                out.push(p.clone());
            }
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            }
        }
    }
    out
}

// --- selection --------------------------------------------------------------

pub fn row_ids(s: &App) -> impl Iterator<Item = &str> {
    s.rows.iter().map(|&i| s.all[i].id.as_str())
}

/// Whether the column view is showing (a search lists its results instead).
pub fn in_columns(s: &App) -> bool {
    s.view == View::Columns && !searching(s)
}

/// The ids the keyboard moves through: the listing, or the column the cursor is in.
pub fn active_ids(s: &App) -> Vec<String> {
    if in_columns(s) {
        let i = active_col(s);
        return s.cols.get(i).map(|c| c.entries.iter().map(|e| col_id(s, &c.dir, &e.name)).collect()).unwrap_or_default();
    }
    row_ids(s).map(str::to_owned).collect()
}

pub fn selected_ids(s: &App) -> Vec<String> {
    if in_columns(s) {
        return s.picks.set.clone();
    }
    // In listing order, as Finder acts on them.
    row_ids(s).filter(|id| s.picks.set.iter().any(|p| p == id)).map(str::to_owned).collect()
}

pub fn selected_len(s: &App) -> usize {
    s.picks.set.len()
}

pub fn is_selected(s: &App, id: &str) -> bool {
    s.picks.set.iter().any(|p| p == id)
}

pub fn selected(s: &App) -> Vec<PathBuf> {
    selected_ids(s).iter().map(|n| s.dir.join(n)).collect()
}

/// The entry for `id`, from the listing or the column view.
pub fn entry<'a>(s: &'a App, id: &str) -> Option<&'a Entry> {
    s.all.iter().find(|e| e.id == id).or_else(|| {
        let p = s.dir.join(id);
        s.cols.iter().filter(|c| Some(c.dir.as_path()) == p.parent()).flat_map(|c| &c.entries).find(|e| p.file_name().is_some_and(|n| n == e.name.as_str()))
    })
}

/// The one item an action on a single item (rename, Get Info) takes.
pub fn focus_id(s: &App) -> Option<String> {
    let ids = selected_ids(s);
    s.picks.cursor.clone().filter(|c| ids.contains(c)).or_else(|| ids.into_iter().next())
}

pub fn select_only(s: &mut App, id: String) {
    s.picks = Picks { set: vec![id.clone()], cursor: Some(id.clone()), anchor: Some(id) };
}

pub fn clear_selection(s: &mut App) {
    s.picks = Picks::default();
}

pub fn select_all(s: &mut App) {
    s.picks.set = active_ids(s);
}

/// The scroll view each view keeps its items in, for `scroll_into_view_in`.
pub fn scroller(s: &App) -> String {
    match s.view {
        _ if searching(s) && s.view == View::Columns => "list".into(),
        View::List => "list".into(),
        View::Icons => "icons".into(),
        View::Gallery => "strip".into(),
        View::Columns => format!("col:{}", active_col(s)),
    }
}

/// Selects `id` and scrolls it into view, in whichever view is showing.
pub fn reveal(s: &mut App, id: String, tx: &mut Tx) {
    if in_columns(s) {
        let p = s.dir.join(&id);
        if let (Some(parent), Some(name)) = (p.parent(), p.file_name()) {
            let name = name.to_string_lossy().into_owned();
            if let Some(i) = s.cols.iter().position(|c| c.dir == parent) {
                col_pick(s, i, &name);
            }
        }
    } else {
        select_only(s, id.clone());
    }
    tx.scroll_into_view_in(scroller(s), id);
}

/// Switches views, keeping the selection.
pub fn set_view(s: &mut App, v: View) {
    if s.view == v {
        return;
    }
    let cursor = focus_id(s);
    s.view = v;
    match v {
        View::Columns => {
            s.cols = vec![Column::read(s.dir.clone(), s.hidden)];
            clear_selection(s);
            // Only an item of this folder has a place in the first column.
            if let Some(c) = cursor.filter(|c| !c.contains('/')) {
                col_pick(s, 0, &c);
            }
        }
        _ => {
            // The outline and groups are the list's alone.
            refresh(s);
            // Column picks deeper than this folder have no row here.
            let (search, list) = (searching(s), v == View::List);
            s.picks.set.retain(|id| !id.contains('/') || search || (list && s.rows.iter().any(|&i| s.all[i].id == *id)));
            if s.picks.cursor.as_ref().is_some_and(|c| !s.picks.set.contains(c)) {
                s.picks.cursor = s.picks.set.first().cloned();
            }
            // The gallery always shows an item.
            if v == View::Gallery && s.picks.set.is_empty() {
                let first = row_ids(s).next().map(str::to_owned);
                if let Some(first) = first {
                    select_only(s, first);
                }
            }
        }
    }
}

/// A click on `id`: Shift extends from the anchor, ⌘ toggles, else selects it alone.
pub fn pick(s: &mut App, id: &str, mods: rune::Mods) {
    if let (true, Some(anchor)) = (mods.shift, s.picks.anchor.clone()) {
        let ids = active_ids(s);
        if let (Some(a), Some(b)) = (ids.iter().position(|i| *i == anchor), ids.iter().position(|i| i == id)) {
            let (lo, hi) = (a.min(b), a.max(b));
            s.picks.set = ids[lo..=hi].to_vec();
            s.picks.cursor = Some(id.to_owned());
            return;
        }
    }
    if mods.cmd {
        if let Some(i) = s.picks.set.iter().position(|p| p == id) {
            s.picks.set.remove(i);
        } else {
            s.picks.set.push(id.to_owned());
        }
        s.picks.cursor = Some(id.to_owned());
        s.picks.anchor = Some(id.to_owned());
        return;
    }
    select_only(s, id.to_owned());
}

/// Moves the cursor by `by` items (a row of a grid is `cols` items), extending with Shift.
pub fn step(s: &mut App, by: isize, extend: bool, tx: &mut Tx) {
    let ids = active_ids(s);
    if ids.is_empty() {
        return;
    }
    let at = s.picks.cursor.as_ref().and_then(|c| ids.iter().position(|i| i == c));
    let next = match at {
        None if by < 0 => ids.len() - 1,
        None => 0,
        Some(i) => (i as isize + by).clamp(0, ids.len() as isize - 1) as usize,
    };
    let id = ids[next].clone();
    if in_columns(s) {
        reveal(s, id, tx);
    } else if extend {
        pick(s, &id, rune::Mods { shift: true, ..Default::default() });
        tx.scroll_into_view_in(scroller(s), id);
    } else {
        select_only(s, id.clone());
        tx.scroll_into_view_in(scroller(s), id);
    }
}

// --- column view ------------------------------------------------------------

/// The id of `name` in `dir`: its path relative to the window's folder.
pub fn col_id(s: &App, dir: &Path, name: &str) -> String {
    let p = dir.join(name);
    p.strip_prefix(&s.dir).map_or_else(|_| p.to_string_lossy().into_owned(), |r| r.to_string_lossy().into_owned())
}

/// The column the keyboard is in: the last one with a pick, else the first.
pub fn active_col(s: &App) -> usize {
    s.cols.iter().rposition(|c| c.pick.is_some()).unwrap_or(0)
}

/// Picks `name` in column `i`: columns right of it close, and a folder opens the next one.
pub fn col_pick(s: &mut App, i: usize, name: &str) {
    if i >= s.cols.len() {
        return;
    }
    s.cols.truncate(i + 1);
    s.cols[i].pick = Some(name.to_owned());
    let dir = s.cols[i].dir.clone();
    let id = col_id(s, &dir, name);
    select_only(s, id);
    let folder = s.cols[i].entries.iter().find(|e| e.name == name).is_some_and(Entry::is_folder);
    if folder {
        s.cols.push(Column::read(dir.join(name), s.hidden));
    }
}

/// → enters the picked folder's column; ← goes back to the column before.
pub fn col_step(s: &mut App, right: bool, tx: &mut Tx) -> bool {
    let i = active_col(s);
    if right {
        let Some(first) = s.cols.get(i + 1).and_then(|c| c.entries.first()).map(|e| e.name.clone()) else { return false };
        col_pick(s, i + 1, &first);
    } else {
        if i == 0 {
            return false;
        }
        let Some(name) = s.cols[i - 1].pick.clone() else { return false };
        col_pick(s, i - 1, &name);
    }
    let id = focus_id(s).unwrap_or_default();
    tx.scroll_into_view_in(scroller(s), id);
    true
}

/// Re-reads every column after a change on disk, keeping the picks that still exist.
pub fn reload_cols(s: &mut App) {
    if s.cols.first().is_none_or(|c| c.dir != s.dir) {
        s.cols = vec![Column::read(s.dir.clone(), s.hidden)];
    }
    let picks: Vec<Option<String>> = s.cols.iter().map(|c| c.pick.clone()).collect();
    let dirs: Vec<PathBuf> = s.cols.iter().map(|c| c.dir.clone()).collect();
    let mut cols = vec![];
    for (dir, pick) in dirs.into_iter().zip(picks) {
        if !dir.is_dir() {
            break;
        }
        let mut c = Column::read(dir, s.hidden);
        c.pick = pick.filter(|p| c.entries.iter().any(|e| &e.name == p));
        let stop = c.pick.is_none();
        cols.push(c);
        if stop {
            break;
        }
    }
    s.cols = cols;
    let keep: Vec<String> = s.picks.set.iter().filter(|id| s.dir.join(id).exists()).cloned().collect();
    s.picks.set = keep;
}

/// Whether a name is being typed (keys less than a second apart), so Space types a space.
pub fn typing(s: &App) -> bool {
    s.typed.1.is_some_and(|t| t.elapsed().as_secs_f32() <= 1.0)
}

/// Type-to-select: letters typed within a second of each other pick the first item whose
/// name starts with them, as Finder does.
pub fn type_select(s: &mut App, t: &str, tx: &mut Tx) -> bool {
    let now = Instant::now();
    let fresh = !typing(s);
    if fresh {
        s.typed.0.clear();
    }
    if fresh && t == " " {
        return false;
    }
    s.typed = (format!("{}{}", s.typed.0, t.to_lowercase()), Some(now));
    let q = s.typed.0.clone();
    let ids = active_ids(s);
    let name = |id: &String| Path::new(id).file_name().map_or_else(|| id.to_lowercase(), |n| n.to_string_lossy().to_lowercase());
    let hit = ids.iter().find(|id| name(id).starts_with(&q)).or_else(|| {
        // Past every match: the first item after where the typed text would sort.
        ids.iter().find(|id| rune_kit::natural_cmp(&name(id), &q).is_gt())
    });
    if let Some(id) = hit.cloned() {
        reveal(s, id, tx);
    }
    true
}

// --- navigation -------------------------------------------------------------

pub fn go(s: &mut App, to: PathBuf) {
    if to == s.dir && !searching(s) {
        return;
    }
    s.back.push(std::mem::replace(&mut s.dir, to));
    s.fwd.clear();
    entered(s);
}

pub fn entered(s: &mut App) {
    s.search = TextEdit::new("");
    s.found.cancel();
    s.renaming = None;
    clear_selection(s);
    s.cols.clear();
    s.expanded.clear();
    load(s);
}

pub fn history(s: &mut App, back: bool) {
    let (from, to) = if back { (&mut s.back, &mut s.fwd) } else { (&mut s.fwd, &mut s.back) };
    if let Some(p) = from.pop() {
        to.push(std::mem::replace(&mut s.dir, p));
        entered(s);
    }
}

/// ⌘↑: the enclosing folder, with the folder just left selected.
pub fn up(s: &mut App, tx: &mut Tx) -> bool {
    let Some(parent) = s.dir.parent().map(Path::to_path_buf) else { return false };
    let child = s.dir.file_name().map(|n| n.to_string_lossy().into_owned());
    go(s, parent);
    if let Some(c) = child {
        reveal(s, c, tx);
    }
    true
}

/// Expands "~" and resolves a path typed into Go to Folder against the current folder.
pub fn resolve(s: &App, typed: &str) -> PathBuf {
    let t = typed.trim();
    if t == "~" {
        home()
    } else if let Some(rest) = t.strip_prefix("~/") {
        home().join(rest)
    } else {
        s.dir.join(t)
    }
}

pub fn display_name(p: &Path) -> String {
    match p.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => "Macintosh HD".into(),
    }
}

pub fn icloud() -> Option<PathBuf> {
    Some(home().join("Library/Mobile Documents/com~apple~CloudDocs")).filter(|p| p.is_dir())
}

/// The id `path` has in the listing: relative to the window's folder (an expanded folder's
/// item is "folder/name"), or the full path for a search result.
pub fn id_for(s: &App, path: &Path) -> String {
    match path.strip_prefix(&s.dir) {
        Ok(r) if !searching(s) => r.to_string_lossy().into_owned(),
        _ => path.to_string_lossy().into_owned(),
    }
}

// --- folder sizes -----------------------------------------------------------

/// Counts every listed folder's size off the UI thread when "모든 크기 계산" is on, sending
/// each as it's done; a newer request stops the older one.
pub fn request_sizes(s: &mut App) {
    if let Some(old) = s.sizer.take() {
        old.stop.store(true, Ordering::Relaxed);
    }
    if !s.calc_sizes || searching(s) {
        return;
    }
    // In the list's order, so the folders on screen fill in first.
    let dirs: Vec<PathBuf> = s.rows.iter().map(|&i| &s.all[i]).filter(|e| e.dir).map(|e| s.dir.join(&e.id)).collect();
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let (halt, waker) = (stop.clone(), s.waker.clone());
    std::thread::spawn(move || {
        for d in dirs {
            let (bytes, _) = crate::ops::tally_until(&d, &halt);
            if halt.load(Ordering::Relaxed) || tx.send((d, bytes)).is_err() {
                return;
            }
            if let Some(w) = &waker {
                w();
            }
        }
    });
    s.sizer = Some(Sizer { rx, stop });
}

/// Takes the sizes counted since the last frame; true when any came in.
pub fn take_sizes(s: &mut App) -> bool {
    let Some(sizer) = &s.sizer else { return false };
    let got: Vec<(PathBuf, u64)> = sizer.rx.try_iter().collect();
    if got.is_empty() {
        return false;
    }
    s.sizes.extend(got);
    if s.sort.0 == "size" || s.group == Group::Size {
        refresh(s);
    }
    true
}

pub fn toggle_sizes(s: &mut App) {
    s.calc_sizes = !s.calc_sizes;
    refresh(s);
    request_sizes(s);
}

// --- favorites --------------------------------------------------------------

fn favorites_file() -> Option<PathBuf> {
    Some(home().join("Library/Application Support/rfinder/favorites"))
}

/// Finder's default favorites, in its order.
pub fn default_favorites() -> Vec<PathBuf> {
    let h = home();
    vec![PathBuf::from("/Applications"), h.join("Desktop"), h.join("Documents"), h.join("Downloads"), h]
}

/// The favorites kept in `file`, one path a line; Finder's defaults until there is one.
pub fn load_favorites(file: Option<&Path>) -> Vec<PathBuf> {
    match file.and_then(|f| std::fs::read_to_string(f).ok()) {
        Some(text) => text.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect(),
        None => default_favorites(),
    }
}

fn save_favorites(s: &mut App) {
    let Some(file) = s.favorites_file.clone() else { return };
    let text: String = s.favorites.iter().map(|p| format!("{}\n", p.display())).collect();
    let r = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&file, text));
    if let Err(e) = r {
        s.error = Some(format!("사이드바를 저장할 수 없음: {e}"));
    }
}

/// ⌃⌘T: the selected folders (or this folder, with nothing selected) join 즐겨찾기.
pub fn add_to_sidebar(s: &mut App) {
    let mut add = selected(s);
    if selected_len(s) == 0 && !searching(s) {
        add.push(s.dir.clone());
    }
    add_favorites(s, &add);
}

/// Folders join 즐겨찾기 at its end, each once; anything else is left out.
pub fn add_favorites(s: &mut App, paths: &[PathBuf]) {
    let before = s.favorites.len();
    for p in paths.iter().filter(|p| p.is_dir()) {
        if !s.favorites.contains(p) {
            s.favorites.push(p.clone());
        }
    }
    if s.favorites.len() != before {
        save_favorites(s);
    }
}

/// A favorite dragged from place `from` to place `to` in the sidebar.
pub fn move_favorite(s: &mut App, from: usize, to: usize) {
    if from >= s.favorites.len() {
        return;
    }
    let p = s.favorites.remove(from);
    s.favorites.insert(to.min(s.favorites.len()), p);
    s.fav_moved = true;
    save_favorites(s);
}

pub fn remove_from_sidebar(s: &mut App, p: &Path) {
    s.favorites.retain(|f| f != p);
    save_favorites(s);
}
