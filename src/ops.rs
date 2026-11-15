//! What Finder's commands do to files: open, rename, duplicate, copy and paste, move, compress,
//! trash, Get Info and Go to Folder.
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use rune::{ClipboardContent, TextEdit, Tx};

use crate::model::{self, App, Entry, Info};

pub fn fail(s: &mut App, what: &str, r: std::io::Result<()>) {
    if let Err(e) = r {
        s.error = Some(format!("{what}: {e}"));
    }
}

fn run(cmd: &str, args: &[&OsStr]) -> std::io::Result<()> {
    let out = Command::new(cmd).args(args).output()?;
    if out.status.success() { Ok(()) } else { Err(std::io::Error::other(String::from_utf8_lossy(&out.stderr).trim().to_owned())) }
}

fn entry<'a>(s: &'a App, id: &str) -> Option<&'a Entry> {
    s.all.iter().find(|e| e.id == id)
}

pub fn open(s: &mut App, id: &str) {
    let p = s.dir.join(id);
    if entry(s, id).is_some_and(Entry::is_folder) || (p.is_dir() && !id.ends_with(".app")) {
        model::go(s, p);
    } else {
        let r = Command::new("open").arg(&p).spawn().map(drop);
        fail(s, "열 수 없음", r);
    }
}

pub fn open_selected(s: &mut App) {
    for id in model::selected_ids(s) {
        open(s, &id);
    }
}

/// ⌘⌫: moves the selection to the Trash, through the system so Put Back works in Finder.
pub fn trash(s: &mut App) {
    let paths = model::selected(s);
    if paths.is_empty() {
        return;
    }
    let args: Vec<&OsStr> = paths.iter().map(|p| p.as_os_str()).collect();
    let r = run("/usr/bin/trash", &args);
    model::clear_selection(s);
    model::load(s);
    fail(s, "휴지통으로 이동할 수 없음", r);
}

/// `name` if nothing in `dir` has it, else "name 2", "name 3", ... before the extension.
pub fn unique_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_owned();
    }
    let (stem, ext) = split_ext(name);
    (2..).map(|n| format!("{stem} {n}{ext}")).find(|n| !dir.join(n).exists()).unwrap_or_default()
}

fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// ⌘D names a duplicate "name 사본", then "name 사본 2", before the extension.
pub fn copy_name(dir: &Path, name: &str) -> String {
    let (stem, ext) = split_ext(name);
    let first = format!("{stem} 사본{ext}");
    if !dir.join(&first).exists() {
        return first;
    }
    (2..).map(|n| format!("{stem} 사본 {n}{ext}")).find(|n| !dir.join(n).exists()).unwrap_or_default()
}

pub fn begin_rename(s: &mut App, id: String) {
    let name = entry(s, &id).map_or_else(|| id.clone(), |e| e.name.clone());
    s.rename = TextEdit::new(name.clone());
    // Select the stem, as Finder does, so typing keeps the extension.
    s.rename.anchor = 0;
    s.rename.focus = name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len());
    s.rename.focused = true;
    s.renaming = Some(id);
}

pub fn begin_rename_selected(s: &mut App) -> bool {
    let ids = model::selected_ids(s);
    match (ids.len(), model::focus_id(s)) {
        (1, Some(id)) => {
            begin_rename(s, id);
            true
        }
        _ => false,
    }
}

pub fn commit_rename(s: &mut App) {
    let Some(id) = s.renaming.take() else { return };
    let old = s.dir.join(&id);
    let new = s.rename.text.trim().to_owned();
    let same = old.file_name().is_some_and(|n| n == OsStr::new(&new));
    if new.is_empty() || same || new.contains('/') {
        return;
    }
    let to = old.with_file_name(&new);
    // A change of case alone is a rename on a case-insensitive volume, not a clash.
    let clash = to.exists() && !old.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&new));
    if clash {
        s.error = Some(format!("“{new}” 이름이 이미 사용 중입니다. 다른 이름을 선택하십시오."));
        return;
    }
    let r = std::fs::rename(&old, &to);
    model::load(s);
    let new_id = if id.contains('/') { to.to_string_lossy().into_owned() } else { new };
    model::select_only(s, new_id);
    fail(s, "이름을 변경할 수 없음", r);
}

pub fn new_folder(s: &mut App) {
    let name = unique_name(&s.dir, "무제 폴더");
    let r = std::fs::create_dir(s.dir.join(&name));
    model::load(s);
    if r.is_ok() {
        model::select_only(s, name.clone());
        begin_rename(s, name);
    }
    fail(s, "폴더를 생성할 수 없음", r);
}

fn copy_into(src: &Path, dst: &Path) -> std::io::Result<()> {
    // `cp -c` clones on APFS, as Finder's copies do; `-R` keeps folders, `-p` dates.
    let flags = if cfg!(target_os = "macos") { "-Rpc" } else { "-Rp" };
    run("/bin/cp", &[flags.as_ref(), src.as_os_str(), dst.as_os_str()])
}

/// ⌘D: a copy of each selected item beside it, selected afterwards.
pub fn duplicate(s: &mut App) {
    let mut made = vec![];
    for src in model::selected(s) {
        let (Some(parent), Some(name)) = (src.parent(), src.file_name().and_then(|n| n.to_str())) else { continue };
        let dst = parent.join(copy_name(parent, name));
        let r = copy_into(&src, &dst);
        if r.is_ok() {
            made.push(dst);
        }
        fail(s, "복제할 수 없음", r);
    }
    let err = s.error.take();
    model::load(s);
    s.error = err;
    if let Some(n) = made.last().and_then(|p| p.file_name()) {
        model::select_only(s, n.to_string_lossy().into_owned());
    }
}

/// ⌘C: the selection on the system clipboard as files, so Finder and other apps paste them,
/// and their names as text.
pub fn copy(s: &mut App, tx: &mut Tx) {
    let paths = model::selected(s);
    if paths.is_empty() {
        return;
    }
    let names: Vec<String> = paths.iter().map(|p| model::display_name(p)).collect();
    tx.copy_rich(ClipboardContent::new(names.join("\r")).with_files(paths.clone()));
    s.clip = paths;
}

/// ⌥⌘C: the selection's paths as text, one per line.
pub fn copy_path(s: &mut App, tx: &mut Tx) {
    let paths = model::selected(s);
    let paths = if paths.is_empty() { vec![s.dir.clone()] } else { paths };
    tx.copy(paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\n"));
}

/// ⌘V pastes the files on the clipboard here; ⌥⌘V moves them instead.
pub fn paste(_: &mut App, mv: bool, tx: &mut Tx) {
    tx.read_clipboard(move |s: &mut App, content, _| {
        let files = if content.files().is_empty() { s.clip.clone() } else { content.files().to_vec() };
        paste_files(s, &files, mv);
    });
}

pub fn paste_files(s: &mut App, files: &[PathBuf], mv: bool) {
    let mut last = None;
    for src in files {
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else { continue };
        let dst = s.dir.join(unique_name(&s.dir, name));
        let r = if mv { move_to(src, &dst) } else { copy_into(src, &dst) };
        if r.is_ok() {
            last = dst.file_name().map(|n| n.to_string_lossy().into_owned());
        }
        fail(s, if mv { "이동할 수 없음" } else { "붙여넣을 수 없음" }, r);
    }
    if mv {
        s.clip.clear();
    }
    let err = s.error.take();
    model::load(s);
    s.error = err;
    if let Some(n) = last {
        model::select_only(s, n);
    }
}

/// `rename`, or a copy and delete across volumes.
fn move_to(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => run("/bin/mv", &[src.as_os_str(), dst.as_os_str()]),
        r => r,
    }
}

fn same_volume(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

/// Files dropped on `folder`: moved within a volume and copied across volumes, as Finder
/// does; ⌥ held copies either way.
pub fn drop_into(s: &mut App, folder: &Path, files: &[PathBuf], copy: bool) {
    let mut moved_any = false;
    for src in files {
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else { continue };
        if src.parent() == Some(folder) || folder.starts_with(src) {
            continue;
        }
        let dst = folder.join(unique_name(folder, name));
        let mv = !copy && same_volume(src, folder);
        let r = if mv { move_to(src, &dst) } else { copy_into(src, &dst) };
        moved_any |= r.is_ok();
        fail(s, "항목을 옮길 수 없음", r);
    }
    if moved_any {
        let err = s.error.take();
        model::load(s);
        s.error = err;
    }
}

/// "압축": one item becomes "name.zip", several become "아카이브.zip", as Finder names them.
pub fn compress(s: &mut App) {
    let paths = model::selected(s);
    let Some(first) = paths.first() else { return };
    let dir = first.parent().unwrap_or(&s.dir).to_path_buf();
    let base = if paths.len() == 1 { format!("{}.zip", model::display_name(first)) } else { "아카이브.zip".into() };
    let out = dir.join(unique_name(&dir, &base));
    let r = if paths.len() == 1 {
        // ditto keeps resource forks and names the archive's top folder as Finder does.
        run("/usr/bin/ditto", &["-c".as_ref(), "-k".as_ref(), "--sequesterRsrc".as_ref(), "--keepParent".as_ref(), first.as_os_str(), out.as_os_str()])
    } else {
        let mut cmd = Command::new("/usr/bin/zip");
        cmd.current_dir(&dir).arg("-qry").arg(&out);
        for p in &paths {
            cmd.arg(p.file_name().unwrap_or_default());
        }
        cmd.output().and_then(|o| if o.status.success() { Ok(()) } else { Err(std::io::Error::other(String::from_utf8_lossy(&o.stderr).trim().to_owned())) })
    };
    model::load(s);
    if let (true, Some(n)) = (r.is_ok(), out.file_name()) {
        model::select_only(s, n.to_string_lossy().into_owned());
    }
    fail(s, "압축할 수 없음", r);
}

/// ⌘I: Get Info for the selection, or for the folder itself with nothing selected.
pub fn get_info(s: &mut App) {
    let (path, entry) = match model::focus_id(s) {
        Some(id) => (s.dir.join(&id), s.all.iter().find(|e| e.id == id).cloned()),
        None => (s.dir.clone(), None),
    };
    let Some(entry) = entry.or_else(|| Entry::read(&path, model::display_name(&path))) else { return };
    let total = (!entry.dir).then_some((entry.size, 1));
    if entry.dir {
        s.totals.request(path.clone(), |p: PathBuf| {
            let (bytes, items) = tally(&p);
            (p, bytes, items)
        });
    }
    s.info = Some(Info { path, entry, total });
}

/// Bytes and items under `root`, counting each hard-linked file once.
pub fn tally(root: &Path) -> (u64, u64) {
    let (mut bytes, mut items, mut stack) = (0u64, 0u64, vec![root.to_path_buf()]);
    let mut seen = std::collections::HashSet::new();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            items += 1;
            if m.is_dir() {
                stack.push(e.path());
            } else if m.nlink() <= 1 || seen.insert((m.dev(), m.ino())) {
                bytes += m.len();
            }
        }
    }
    (bytes, items)
}

/// Go to Folder: a folder opens; a file opens its folder with the file selected.
pub fn goto(s: &mut App, typed: &str, tx: &mut Tx) {
    let p = model::resolve(s, typed);
    if p.is_dir() {
        s.goto = None;
        model::go(s, p);
    } else if let (true, Some(parent)) = (p.exists(), p.parent()) {
        s.goto = None;
        model::go(s, parent.to_path_buf());
        model::reveal(s, model::display_name(&p), tx);
    } else {
        s.error = Some(format!("“{}” 폴더를 찾을 수 없습니다.", typed.trim()));
    }
}

pub fn eject(s: &mut App, volume: &Path) {
    let r = run("/usr/sbin/diskutil", &["eject".as_ref(), volume.as_os_str()]);
    if r.is_ok() {
        s.volumes.retain(|v| v != volume);
        if s.dir.starts_with(volume) {
            model::go(s, model::home());
        }
    }
    fail(s, "추출할 수 없음", r);
}

/// "둘러싸는 폴더에서 보기": leaves a search for the item's own folder, with it selected.
pub fn show_in_folder(s: &mut App, tx: &mut Tx) {
    let Some(id) = model::focus_id(s) else { return };
    let p = s.dir.join(&id);
    if let Some(parent) = p.parent() {
        model::go(s, parent.to_path_buf());
        model::reveal(s, model::display_name(&p), tx);
    }
}

pub fn quick_look(s: &mut App) {
    let paths = model::selected(s);
    if !paths.is_empty() {
        let r = Command::new("/usr/bin/qlmanage").arg("-p").args(&paths).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(drop);
        fail(s, "훑어볼 수 없음", r);
    }
}

pub fn toggle_hidden(s: &mut App) {
    s.hidden ^= true;
    if model::searching(s) { model::request_search(s) } else { model::refresh(s) }
}
