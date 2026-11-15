//! The window's state: the folder shown, its items, the selection, history and search.
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use rune::{Latest, PointerPress, TextEdit, Tx};
use rune_kit::shell::PaneState;
use rune_kit::{SortDir, TableState};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Icons,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Spotlight's whole Mac, Finder's default.
    Mac,
    /// The folder the search started in, and everything inside it.
    Folder,
}

/// The icon view's selection (the list view's lives in its `TableState`).
#[derive(Default)]
pub struct Picks {
    pub set: Vec<String>,
    pub cursor: Option<String>,
    pub anchor: Option<String>,
}

/// Get Info's window: one item, with its folder size filled in once it's counted.
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
    pub table: TableState,
    pub picks: Picks,
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
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

pub fn init() -> App {
    let mut table = TableState::new().multi_select(true);
    table.set_sort("name", SortDir::Ascending);
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
        table,
        picks: Picks::default(),
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
    };
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
    s.all = match std::fs::read_dir(&s.dir) {
        Ok(d) => d.flatten().filter_map(|e| Entry::read(&e.path(), e.file_name().to_string_lossy().into_owned())).collect(),
        Err(e) => {
            s.error = Some(format!("폴더를 열 수 없음: {e}"));
            vec![]
        }
    };
    refresh(s);
}

/// Rebuilds `rows` from `all`: hidden files out, then the sort.
pub fn refresh(s: &mut App) {
    let all = &s.all;
    s.rows = (0..all.len()).filter(|&i| s.hidden || !all[i].name.starts_with('.')).collect();
    let (column, dir) = s.table.sort().map_or(("name", SortDir::Ascending), |(c, d)| (c, d));
    s.rows.sort_by(|&a, &b| {
        let (a, b) = (&all[a], &all[b]);
        let name = || rune_kit::natural_cmp(&a.name, &b.name);
        dir.apply(match column {
            "modified" => a.modified.cmp(&b.modified).then_with(name),
            "size" => a.size.cmp(&b.size).then_with(name),
            "kind" => a.kind.cmp(&b.kind).then_with(name),
            _ => name(),
        })
    });
    // Drop selected ids that left the listing.
    let present = |id: &str| s.rows.iter().any(|&i| all[i].id == id);
    if s.table.selected().any(|id| !present(id)) {
        s.table.clear_selection();
    }
    s.picks.set.retain(|id| s.rows.iter().any(|&i| all[i].id == *id));
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

pub fn selected_ids(s: &App) -> Vec<String> {
    match s.view {
        View::List => {
            // In listing order, as Finder acts on them.
            row_ids(s).filter(|id| s.table.is_selected(id)).map(str::to_owned).collect()
        }
        View::Icons => row_ids(s).filter(|id| s.picks.set.iter().any(|p| p == id)).map(str::to_owned).collect(),
    }
}

pub fn selected_len(s: &App) -> usize {
    match s.view {
        View::List => s.table.selected_len(),
        View::Icons => s.picks.set.len(),
    }
}

pub fn is_selected(s: &App, id: &str) -> bool {
    match s.view {
        View::List => s.table.is_selected(id),
        View::Icons => s.picks.set.iter().any(|p| p == id),
    }
}

pub fn selected(s: &App) -> Vec<PathBuf> {
    selected_ids(s).iter().map(|n| s.dir.join(n)).collect()
}

/// The one item an action on a single item (rename, Get Info) takes.
pub fn focus_id(s: &App) -> Option<String> {
    let ids = selected_ids(s);
    match s.view {
        View::List => s.table.cursor().filter(|c| ids.iter().any(|i| i == c)).map(str::to_owned),
        View::Icons => s.picks.cursor.clone().filter(|c| ids.contains(c)),
    }
    .or_else(|| ids.into_iter().next())
}

pub fn select_only(s: &mut App, id: String) {
    s.table.select(id.clone());
    s.picks = Picks { set: vec![id.clone()], cursor: Some(id.clone()), anchor: Some(id) };
}

pub fn clear_selection(s: &mut App) {
    s.table.clear_selection();
    s.picks = Picks::default();
}

pub fn select_all(s: &mut App) {
    s.picks.set = row_ids(s).map(str::to_owned).collect();
    // The table selects everything itself on ⌘A while it has focus; from elsewhere the
    // closest it offers is the cursor's row.
    if let Some(first) = s.picks.set.first().cloned() {
        s.table.select(first);
    }
}

/// Selects `id` and scrolls it into view, in whichever view is showing.
pub fn reveal(s: &mut App, id: String, tx: &mut Tx) {
    select_only(s, id.clone());
    match s.view {
        View::List => s.table.reveal(id, tx),
        View::Icons => tx.scroll_into_view_in("icons", id),
    }
}

/// Switches views, keeping as much of the selection as the other view can hold.
pub fn set_view(s: &mut App, v: View) {
    if s.view == v {
        return;
    }
    let ids = selected_ids(s);
    let cursor = focus_id(s);
    s.view = v;
    match v {
        View::Icons => s.picks = Picks { set: ids, cursor: cursor.clone(), anchor: cursor },
        View::List => {
            s.table.clear_selection();
            if let Some(c) = cursor {
                s.table.select(c);
            }
        }
    }
}

/// An icon-view click on `id`: Shift extends from the anchor, ⌘ toggles, else selects it alone.
pub fn pick(s: &mut App, id: &str, mods: rune::Mods) {
    if let (true, Some(anchor)) = (mods.shift, s.picks.anchor.clone()) {
        let ids: Vec<&str> = row_ids(s).collect();
        if let (Some(a), Some(b)) = (ids.iter().position(|i| *i == anchor), ids.iter().position(|i| *i == id)) {
            let (lo, hi) = (a.min(b), a.max(b));
            s.picks.set = ids[lo..=hi].iter().map(|i| (*i).to_owned()).collect();
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

/// Moves the icon view's cursor by `dx` columns and `dy` rows of `cols` per row.
pub fn step_picks(s: &mut App, dx: isize, dy: isize, cols: usize, extend: bool, tx: &mut Tx) {
    let ids: Vec<String> = row_ids(s).map(str::to_owned).collect();
    if ids.is_empty() {
        return;
    }
    let at = s.picks.cursor.as_ref().and_then(|c| ids.iter().position(|i| i == c));
    let next = match at {
        None => 0,
        Some(i) => (i as isize + dx + dy * cols.max(1) as isize).clamp(0, ids.len() as isize - 1) as usize,
    };
    let id = ids[next].clone();
    if extend {
        pick(s, &id, rune::Mods { shift: true, ..Default::default() });
    } else {
        select_only(s, id.clone());
    }
    tx.scroll_into_view_in("icons", id);
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
    let hit = s.rows.iter().map(|&i| &s.all[i]).find(|e| e.name.to_lowercase().starts_with(&q)).or_else(|| {
        // Past every match: the first item after where the typed text would sort.
        s.rows.iter().map(|&i| &s.all[i]).find(|e| rune_kit::natural_cmp(&e.name.to_lowercase(), &q).is_gt())
    });
    if let Some(id) = hit.map(|e| e.id.clone()) {
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
