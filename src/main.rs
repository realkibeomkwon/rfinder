//! rfinder: a small, fast Finder-style file browser built on Rune.
//! `cargo run --release` opens the window; `-- --snap out.png` renders it headless.
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use rune::{Align, AppSpec, Chrome, Cx, El, Icon, Key, KeyEvent, PointerPress, TextEdit, Tx, col, el, icon, oklch, row, text};
use rune_kit::shell::{self, PaneState, Sidebar, SidebarRow, SidebarSection, SplitView, Toolbar};
use rune_kit::{Cell, IconButton, IconButtonVariant, Kit, MenuItem, SortDir, Table, TableColumn, TableState, TextInput, context_menu, glyphs};

static BACK: Icon = Icon {
    name: "rf-back",
    view: 16.0,
    body: r#"<path d="M10 3L5 8L10 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
static FORWARD: Icon = Icon {
    name: "rf-forward",
    view: 16.0,
    body: r#"<path d="M6 3L11 8L6 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
static DISK: Icon = Icon {
    name: "rf-disk",
    view: 16.0,
    body: r#"<rect x="2.5" y="4.5" width="11" height="7" rx="1.5" stroke="currentColor" stroke-width="1.2" fill="none"/><circle cx="11" cy="8" r="0.9" fill="currentColor"/>"#,
};

struct Entry {
    name: String,
    dir: bool,
    size: u64,
    modified: i64,
    kind: String,
}

struct App {
    sidebar: PaneState,
    dir: PathBuf,
    back: Vec<PathBuf>,
    fwd: Vec<PathBuf>,
    all: Vec<Entry>,
    /// Indices into `all` after the search filter, in sort order.
    rows: Vec<usize>,
    table: TableState,
    search: TextEdit,
    hidden: bool,
    menu: Option<PointerPress>,
    renaming: Option<String>,
    rename: TextEdit,
    clip: Vec<PathBuf>,
    volumes: Vec<PathBuf>,
    error: Option<String>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn init() -> App {
    let mut table = TableState::new().multi_select(true);
    table.set_sort("name", SortDir::Ascending);
    let start = rune::extra_args().iter().map(PathBuf::from).find(|p| p.is_dir()).unwrap_or_else(home);
    let volumes = std::fs::read_dir("/Volumes").map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    let mut s = App {
        sidebar: PaneState::new(190.0).range(140.0, 400.0),
        dir: start,
        back: vec![],
        fwd: vec![],
        all: vec![],
        rows: vec![],
        table,
        search: TextEdit::new(""),
        hidden: false,
        menu: None,
        renaming: None,
        rename: TextEdit::new(""),
        clip: vec![],
        volumes,
        error: None,
    };
    load(&mut s);
    s
}

// --- model -----------------------------------------------------------------

fn kind(name: &str, dir: bool) -> String {
    if dir {
        return if name.ends_with(".app") { "응용 프로그램".into() } else { "폴더".into() };
    }
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{} 파일", ext.to_uppercase()),
        None => "문서".into(),
    }
}

// shortcut: reads the folder on the UI thread (about 20 ms per 10,000 entries on an SSD);
// move to `rune::Latest` if network volumes or huge folders stall the window.
fn load(s: &mut App) {
    s.error = None;
    s.all = match std::fs::read_dir(&s.dir) {
        Ok(d) => d
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                // `fs::metadata` follows symlinks, so a link to a folder opens as one.
                let m = std::fs::metadata(e.path()).or_else(|_| e.metadata()).ok();
                let dir = m.as_ref().is_some_and(|m| m.is_dir());
                let modified = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                Entry { kind: kind(&name, dir), size: m.map_or(0, |m| m.len()), name, dir, modified }
            })
            .collect(),
        Err(e) => {
            s.error = Some(format!("폴더를 열 수 없음: {e}"));
            vec![]
        }
    };
    refresh(s);
}

/// Rebuilds `rows` from `all`: hidden files, the search filter, then the sort.
fn refresh(s: &mut App) {
    let q = s.search.text.to_lowercase();
    let all = &s.all;
    s.rows = (0..all.len()).filter(|&i| (s.hidden || !all[i].name.starts_with('.')) && (q.is_empty() || all[i].name.to_lowercase().contains(&q))).collect();
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
    let keep: Vec<String> = s.table.selected().filter(|id| s.rows.iter().any(|&i| all[i].name == *id)).map(str::to_owned).collect();
    if keep.len() != s.table.selected_len() {
        s.table.clear_selection();
    }
}

fn go(s: &mut App, to: PathBuf) {
    if to == s.dir {
        return;
    }
    s.back.push(std::mem::replace(&mut s.dir, to));
    s.fwd.clear();
    entered(s);
}

fn entered(s: &mut App) {
    s.search = TextEdit::new("");
    s.renaming = None;
    s.table.clear_selection();
    load(s);
}

fn history(s: &mut App, back: bool) {
    let (from, to) = if back { (&mut s.back, &mut s.fwd) } else { (&mut s.fwd, &mut s.back) };
    if let Some(p) = from.pop() {
        to.push(std::mem::replace(&mut s.dir, p));
        entered(s);
    }
}

fn selected(s: &App) -> Vec<PathBuf> {
    s.table.selected().map(|n| s.dir.join(n)).collect()
}

fn fail(s: &mut App, what: &str, r: std::io::Result<()>) {
    if let Err(e) = r {
        s.error = Some(format!("{what}: {e}"));
    }
}

fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> std::io::Result<()> {
    let out = Command::new(cmd).args(args).output()?;
    if out.status.success() { Ok(()) } else { Err(std::io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_owned())) }
}

fn open(s: &mut App, name: &str) {
    let p = s.dir.join(name);
    // A bundle (.app) is a folder that Finder launches instead of entering.
    if p.is_dir() && !name.ends_with(".app") {
        go(s, p);
    } else {
        let r = Command::new("open").arg(&p).spawn().map(drop);
        fail(s, "열 수 없음", r);
    }
}

fn open_selected(s: &mut App) {
    let names: Vec<String> = s.table.selected().map(str::to_owned).collect();
    for n in names {
        open(s, &n);
    }
}

fn trash(s: &mut App) {
    let paths = selected(s);
    if paths.is_empty() {
        return;
    }
    // The system `trash` tool moves items to the Trash, so Put Back still works in Finder.
    let args: Vec<&std::ffi::OsStr> = paths.iter().map(|p| p.as_os_str()).collect();
    let r = run("/usr/bin/trash", &args);
    s.table.clear_selection();
    load(s);
    fail(s, "휴지통으로 이동할 수 없음", r);
}

/// `name` if nothing in `dir` has it, else "name 2", "name 3", ... before the extension.
fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_owned();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..).map(|n| format!("{stem} {n}{ext}")).find(|n| !dir.join(n).exists()).unwrap_or_default()
}

fn begin_rename(s: &mut App, name: String) {
    s.rename = TextEdit::new(name.clone());
    // Select the stem, as Finder does, so typing keeps the extension.
    s.rename.anchor = 0;
    s.rename.focus = name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len());
    s.rename.focused = true;
    s.renaming = Some(name);
}

fn commit_rename(s: &mut App) {
    let Some(old) = s.renaming.take() else { return };
    let new = s.rename.text.trim().to_owned();
    if new.is_empty() || new == old || new.contains('/') {
        return;
    }
    if s.dir.join(&new).exists() {
        s.error = Some(format!("“{new}” 이름이 이미 사용 중입니다."));
        return;
    }
    let r = std::fs::rename(s.dir.join(&old), s.dir.join(&new));
    load(s);
    s.table.select(new);
    fail(s, "이름을 변경할 수 없음", r);
}

fn new_folder(s: &mut App) {
    let name = unique_name(&s.dir, "무제 폴더");
    let r = std::fs::create_dir(s.dir.join(&name));
    load(s);
    if r.is_ok() {
        s.table.select(name.clone());
        begin_rename(s, name);
    }
    fail(s, "폴더를 생성할 수 없음", r);
}

/// Pastes the copied items here; `mv` moves them instead (Finder's ⌥⌘V).
fn paste(s: &mut App, mv: bool) {
    let clip = std::mem::take(&mut s.clip);
    for src in &clip {
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else { continue };
        let dst = s.dir.join(unique_name(&s.dir, name));
        let r = if mv { std::fs::rename(src, &dst) } else { run("/bin/cp", &["-Rp".as_ref(), src.as_os_str(), dst.as_os_str()]) };
        fail(s, "붙여넣을 수 없음", r);
    }
    if !mv {
        s.clip = clip;
    }
    load(s);
}

fn quick_look(s: &mut App) {
    let paths = selected(s);
    if !paths.is_empty() {
        let r = Command::new("/usr/bin/qlmanage").arg("-p").args(&paths).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(drop);
        fail(s, "훑어볼 수 없음", r);
    }
}

// --- formatting ------------------------------------------------------------

fn fmt_size(b: u64) -> String {
    // Finder counts in powers of 1000.
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if b < 1000 {
        return format!("{b}바이트");
    }
    let mut v = b as f64 / 1000.0;
    let mut u = 0;
    while v >= 1000.0 && u < 3 {
        v /= 1000.0;
        u += 1;
    }
    if v < 10.0 && u > 0 { format!("{v:.1}{}", UNITS[u]) } else { format!("{v:.0}{}", UNITS[u]) }
}

fn fmt_date(secs: i64) -> String {
    if secs == 0 {
        return "--".into();
    }
    // SAFETY: `tm` is plain data that `localtime_r` fills; both pointers are valid for the call.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&(secs as libc::time_t), &mut tm);
        tm
    };
    let (h, ampm) = (tm.tm_hour, if tm.tm_hour < 12 { "오전" } else { "오후" });
    format!("{}. {}. {}. {ampm} {}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, if h % 12 == 0 { 12 } else { h % 12 }, tm.tm_min)
}

fn display_name(p: &Path) -> String {
    match p.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => "Macintosh HD".into(),
    }
}

// --- view ------------------------------------------------------------------

fn sidebar(s: &App, k: &Kit) -> El {
    let place = |label: &str, glyph: &'static Icon, p: PathBuf| {
        let to = p.clone();
        SidebarRow::new(label.to_owned())
            .icon(glyph)
            .height(28.0)
            .selected(s.dir == p)
            .on_select(move |s: &mut App, _| go(s, to.clone()))
            .build(k)
            .key(format!("place:{}", p.display()))
    };
    let h = home();
    let favorites = SidebarSection::new("즐겨찾기").rows([
        place(&display_name(&h), &glyphs::FOLDER, h.clone()),
        place("데스크탑", &glyphs::FOLDER, h.join("Desktop")),
        place("문서", &glyphs::FOLDER, h.join("Documents")),
        place("다운로드", &glyphs::FOLDER, h.join("Downloads")),
        place("응용 프로그램", &glyphs::FOLDER, PathBuf::from("/Applications")),
    ]);
    let mut places = SidebarSection::new("위치").row(place("Macintosh HD", &DISK, PathBuf::from("/")));
    // "/Volumes/Macintosh HD" is a link back to "/".
    for v in s.volumes.iter().filter(|v| !std::fs::symlink_metadata(v).is_ok_and(|m| m.file_type().is_symlink())) {
        places = places.row(place(&display_name(v), &DISK, v.clone()));
    }
    Sidebar::new().label("사이드바").section(favorites).section(places).build(k)
}

fn file_table(s: &App, k: &Kit) -> El {
    let kit = k.clone();
    Table::new(|s: &App| &s.table, |s: &mut App| &mut s.table, |s: &App| s.rows.len(), |s: &App, i| Cow::Borrowed(s.all[s.rows[i]].name.as_str()))
        .name("파일")
        .column(TableColumn::new("name", "이름").flex(3.0).min(180.0).sortable())
        .column(TableColumn::new("modified", "수정일").width(190.0).min(120.0).sortable())
        .column(TableColumn::new("size", "크기").width(90.0).min(60.0).align(Align::End).sortable())
        .column(TableColumn::new("kind", "종류").width(130.0).min(80.0).sortable())
        .cell(move |s: &App, i, column| {
            let e = &s.all[s.rows[i]];
            match column {
                "name" => {
                    let k = &kit;
                    let glyph = if e.dir && !e.name.ends_with(".app") { icon(&glyphs::FOLDER, 16.0).color(oklch(0.72, 0.13, 240.0)) } else { icon(&glyphs::FILE, 16.0).color(k.c.icon_muted) };
                    let label = if s.renaming.as_deref() == Some(e.name.as_str()) {
                        TextInput::new(&s.rename)
                            .label("이름 변경")
                            .height(20.0)
                            .on_edit(|s: &mut App| &mut s.rename)
                            .on_submit(|s: &mut App, _| commit_rename(s))
                            .on_key(|s: &mut App, ev: &KeyEvent, _| {
                                let esc = ev.key == Key::Escape;
                                if esc {
                                    s.renaming = None;
                                }
                                esc
                            })
                            .build(k)
                            .grow(1.0)
                    } else {
                        text(e.name.clone()).font(k.ty.ui).color(k.c.text_base)
                    };
                    Cell::el(row().w_full().gap(6.0).center_y().child(glyph).child(label))
                }
                "modified" => Cell::text(fmt_date(e.modified)).muted(),
                "size" => Cell::text(if e.dir { "--".into() } else { fmt_size(e.size) }).muted(),
                _ => Cell::text(e.kind.clone()).muted(),
            }
        })
        .on_sort(|s: &mut App, _, _, _| refresh(s))
        .on_open(|s: &mut App, id, _| open(s, id))
        .zebra(true)
        .build(s, k)
        .grow(1.0)
        .min_h(0.0)
}

fn path_bar(s: &App, k: &Kit) -> El {
    let mut bar = row().key("pathbar").w_full().h(24.0).px(10.0).gap(4.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak);
    let crumbs: Vec<&Path> = s.dir.ancestors().collect();
    for (i, p) in crumbs.iter().rev().enumerate() {
        if i > 0 {
            bar = bar.child(icon(&glyphs::CARET_RIGHT, 10.0).color(k.c.icon_faint));
        }
        let to = p.to_path_buf();
        bar = bar.child(
            text(display_name(p))
                .key(format!("crumb:{i}"))
                .font(k.ty.text_small)
                .color(k.c.text_muted)
                .on_press(move |s: &mut App| go(s, to.clone())),
        );
    }
    bar
}

fn status(s: &App, k: &Kit) -> El {
    let (label, ink) = match &s.error {
        Some(e) => (e.clone(), k.c.danger.icon),
        None => match s.table.selected_len() {
            0 => (format!("{}개 항목", s.rows.len()), k.c.text_muted),
            n => (format!("{}개 중 {n}개 선택됨", s.rows.len()), k.c.text_muted),
        },
    };
    row().key("status").w_full().h(22.0).center().border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak).child(text(label).font(k.ty.text_small).color(ink))
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let mut layer = Vec::new();
    let some = s.table.selected_len() > 0;
    let one = s.table.selected_len() == 1;

    let files = col().key("files").w_full().grow(1.0).min_h(0.0).child(file_table(s, &k)).on_secondary_press(|s: &mut App, p, _| s.menu = Some(p));
    // shortcut: the menu acts on the current selection; a right-click does not select the row
    // under the pointer. Needs the row index from Table to upgrade.
    let item = |label: &str, keys: &str, on: bool, f: fn(&mut App)| {
        MenuItem::new(label.to_owned()).shortcut(keys.to_owned()).disabled(!on).on_select(move |s: &mut App| {
            s.menu = None;
            f(s);
        })
    };
    // The menu opens at the pointer, so its trigger is an empty anchor outside the list's layout.
    let menu = context_menu("file-menu", el(), s.menu)
        .item(item("열기", "⌘↓", some, open_selected))
        .item(item("훑어보기", "Space", some, quick_look))
        .separator()
        .item(item("이름 변경", "F2", one, |s| {
            if let Some(n) = s.table.cursor().map(str::to_owned) {
                begin_rename(s, n);
            }
        }))
        .item(item("복사", "⌘C", some, |s| s.clip = selected(s)))
        .item(item("붙여넣기", "⌘V", !s.clip.is_empty(), |s| paste(s, false)))
        .item(item("휴지통으로 이동", "⌘⌫", some, trash))
        .separator()
        .item(item("새로운 폴더", "⇧⌘N", true, new_folder))
        .item(item(if s.hidden { "숨김 파일 가리기" } else { "숨김 파일 보기" }, "⇧⌘.", true, |s| {
            s.hidden ^= true;
            refresh(s);
        }))
        .on_close(|s: &mut App| s.menu = None)
        .build(&k, &mut layer);

    let main = col().key("main").w_full().h_full().min_h(0.0).child(files).child(menu).child(path_bar(s, &k)).child(status(s, &k));

    let nav = |glyph: &'static Icon, label: &str, on: bool, back: bool| {
        IconButton::new(glyph)
            .variant(IconButtonVariant::GhostMuted)
            .label(label.to_owned())
            .disabled(!on)
            .on_press(move |s: &mut App| history(s, back))
            .build(&k)
            .key(label.to_owned())
    };
    let search = TextInput::new(&s.search)
        .label("검색")
        .placeholder("검색")
        .leading_icon(&glyphs::SEARCH)
        .width(Some(180.0))
        .clear_button()
        .on_edit(|s: &mut App| &mut s.search)
        .on_change(|s: &mut App, _| refresh(s))
        .build(&k)
        .key("search");
    let toolbar = Toolbar::new()
        .leading(row().gap(2.0).child(nav(&BACK, "뒤로", !s.back.is_empty(), true)).child(nav(&FORWARD, "앞으로", !s.fwd.is_empty(), false)))
        .title(display_name(&s.dir))
        .trailing(search);

    let shell = SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, sidebar(s, &k))
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .reduced_motion(cx.is_reduced_motion())
        .build(&k);
    rune_kit::root(shell, layer)
}

fn on_key(s: &mut App, ev: &KeyEvent, _: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        return true;
    }
    let m = ev.mods;
    match &ev.key {
        Key::Up if m.cmd => match s.dir.parent().map(Path::to_path_buf) {
            Some(p) => go(s, p),
            None => return false,
        },
        Key::Down if m.cmd => open_selected(s),
        Key::Backspace if m.cmd => trash(s),
        Key::Function(2) => match s.table.cursor().map(str::to_owned) {
            Some(n) => begin_rename(s, n),
            None => return false,
        },
        Key::Text(t) if t == " " && !m.cmd => quick_look(s),
        Key::Command('[') => history(s, true),
        Key::Command(']') => history(s, false),
        Key::Command('o') => open_selected(s),
        Key::Command('r') => load(s),
        Key::Command('c') => s.clip = selected(s),
        Key::Command('v') => paste(s, m.alt),
        Key::Command('n') if m.shift => new_folder(s),
        Key::Command('.') if m.shift => {
            s.hidden ^= true;
            refresh(s);
        }
        Key::Command('f') => s.search.focused = true,
        _ => return false,
    }
    true
}

fn main() {
    let chrome = Chrome { full_size_content: true, traffic_lights: Some((16.0, 17.0)), drag_regions: true };
    rune::App::new(AppSpec { on_key: Some(on_key), min_size: Some((560.0, 320.0)), ..AppSpec::new("rfinder", (920.0, 560.0), init, view) }).chrome(chrome).run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_powers_of_1000() {
        assert_eq!(fmt_size(999), "999바이트");
        // Finder shows KB whole and larger units with one decimal below 10.
        assert_eq!(fmt_size(1500), "2KB");
        assert_eq!(fmt_size(1_500_000), "1.5MB");
        assert_eq!(fmt_size(25_000_000), "25MB");
        assert_eq!(fmt_size(3_200_000_000), "3.2GB");
    }

    #[test]
    fn unique_name_counts_before_the_extension() {
        let dir = std::env::temp_dir().join(format!("rfinder-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_name(&dir, "a.txt"), "a.txt");
        std::fs::write(dir.join("a.txt"), "").unwrap();
        assert_eq!(unique_name(&dir, "a.txt"), "a 2.txt");
        std::fs::write(dir.join("a 2.txt"), "").unwrap();
        assert_eq!(unique_name(&dir, "a.txt"), "a 3.txt");
        std::fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(unique_name(&dir, ".git"), ".git 2");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn listing_filters_hidden_and_search_and_sorts_naturally() {
        let dir = std::env::temp_dir().join(format!("rfinder-list-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for n in ["file10.txt", "file2.txt", ".hidden", "Other.md"] {
            std::fs::write(dir.join(n), "x").unwrap();
        }
        let mut s = init();
        s.dir = dir.clone();
        load(&mut s);
        let names = |s: &App| s.rows.iter().map(|&i| s.all[i].name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&s), ["file2.txt", "file10.txt", "Other.md", "sub"]);
        s.hidden = true;
        s.search = TextEdit::new("FILE");
        refresh(&mut s);
        assert_eq!(names(&s), ["file2.txt", "file10.txt"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
