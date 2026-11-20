use super::*;
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rfinder-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn app_in(dir: &Path) -> App {
    let mut s = model::init();
    s.dir = dir.to_path_buf();
    model::load(&mut s);
    s
}

fn names(s: &App) -> Vec<String> {
    s.rows.iter().map(|&i| s.all[i].name.clone()).collect()
}

#[test]
fn sizes_use_powers_of_1000() {
    assert_eq!(format::size(999), "999바이트");
    // Finder shows KB whole and larger units with one decimal below 10.
    assert_eq!(format::size(1500), "2KB");
    assert_eq!(format::size(1_500_000), "1.5MB");
    assert_eq!(format::size(25_000_000), "25MB");
    assert_eq!(format::size(3_200_000_000), "3.2GB");
    assert_eq!(format::free(431_270_000_000), "431.27GB");
    assert_eq!(format::bytes(1_234_567), "1,234,567바이트");
    assert_eq!(format::bytes(999), "999바이트");
}

#[test]
fn dates_say_today_and_yesterday() {
    // Noon local time today, wherever the test runs.
    let now = {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        // SAFETY: plain data filled by `localtime_r`.
        let off = unsafe {
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&(t as libc::time_t), &mut tm);
            tm.tm_gmtoff as i64
        };
        (t + off).div_euclid(86_400) * 86_400 - off + 12 * 3600
    };
    assert_eq!(format::date_at(now - 3600 + 42 * 60, now), "오늘 오전 11:42");
    assert_eq!(format::date_at(now + 3 * 3600 + 5 * 60, now), "오늘 오후 3:05");
    assert_eq!(format::date_at(now - 24 * 3600, now), "어제 오후 12:00");
    assert!(format::date_at(now - 3 * 24 * 3600, now).contains("년 "));
    assert_eq!(format::date_at(0, now), "--");
}

#[test]
fn kinds_read_like_finder() {
    assert_eq!(format::kind("a.png", false, false), "PNG 이미지");
    assert_eq!(format::kind("Notes.TXT", false, false), "일반 텍스트 문서");
    assert_eq!(format::kind("x.app", true, false), "응용 프로그램");
    assert_eq!(format::kind("src", true, false), "폴더");
    assert_eq!(format::kind("link", false, true), "별칭");
    assert_eq!(format::kind("data.xyz", false, false), "XYZ 문서");
    assert_eq!(format::kind("Makefile", false, false), "문서");
}

#[test]
fn unique_and_copy_names_count_before_the_extension() {
    let dir = scratch("names");
    assert_eq!(ops::unique_name(&dir, "a.txt"), "a.txt");
    std::fs::write(dir.join("a.txt"), "").unwrap();
    assert_eq!(ops::unique_name(&dir, "a.txt"), "a 2.txt");
    std::fs::write(dir.join("a 2.txt"), "").unwrap();
    assert_eq!(ops::unique_name(&dir, "a.txt"), "a 3.txt");
    std::fs::create_dir(dir.join(".git")).unwrap();
    assert_eq!(ops::unique_name(&dir, ".git"), ".git 2");
    assert_eq!(ops::copy_name(&dir, "a.txt"), "a 사본.txt");
    std::fs::write(dir.join("a 사본.txt"), "").unwrap();
    assert_eq!(ops::copy_name(&dir, "a.txt"), "a 사본 2.txt");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn listing_hides_dot_files_and_sorts_naturally() {
    let dir = scratch("list");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    for n in ["file10.txt", "file2.txt", ".hidden", "Other.md"] {
        std::fs::write(dir.join(n), "x").unwrap();
    }
    let mut s = app_in(&dir);
    assert_eq!(names(&s), ["file2.txt", "file10.txt", "Other.md", "sub"]);
    s.hidden = true;
    model::refresh(&mut s);
    assert_eq!(names(&s), [".hidden", "file2.txt", "file10.txt", "Other.md", "sub"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn search_finds_names_in_subfolders() {
    let dir = scratch("find");
    std::fs::create_dir_all(dir.join("a/b")).unwrap();
    std::fs::write(dir.join("a/b/Report.pdf"), "x").unwrap();
    std::fs::write(dir.join("report-old.txt"), "x").unwrap();
    std::fs::write(dir.join("a/.report"), "x").unwrap();
    std::fs::write(dir.join("other.txt"), "x").unwrap();
    let mut found: Vec<String> = model::find("report", Some(&dir), false).into_iter().map(|e| e.name).collect();
    found.sort();
    assert_eq!(found, ["Report.pdf", "report-old.txt"]);
    // Results carry their full path as id, so `dir.join(id)` still finds them.
    let hit = model::find("Report.pdf", Some(&dir), false).remove(0);
    assert_eq!(dir.join(&hit.id), dir.join("a/b/Report.pdf"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn duplicate_selects_the_copy() {
    let dir = scratch("dup");
    std::fs::write(dir.join("doc.txt"), "hello").unwrap();
    let mut s = app_in(&dir);
    model::select_only(&mut s, "doc.txt".into());
    ops::duplicate(&mut s);
    assert_eq!(names(&s), ["doc 사본.txt", "doc.txt"]);
    assert_eq!(model::selected_ids(&s), ["doc 사본.txt"]);
    assert_eq!(std::fs::read_to_string(dir.join("doc 사본.txt")).unwrap(), "hello");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn drops_move_within_a_volume_and_skip_their_own_folder() {
    let dir = scratch("drop");
    std::fs::create_dir_all(dir.join("into")).unwrap();
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    ops::drop_into(&mut s, &dir.join("into"), &[dir.join("a.txt")], false);
    assert!(dir.join("into/a.txt").exists() && !dir.join("a.txt").exists());
    assert_eq!(names(&s), ["into"]);
    // A folder dropped on itself or into its own folder stays put.
    ops::drop_into(&mut s, &dir.join("into"), &[dir.join("into")], false);
    ops::drop_into(&mut s, &dir.join("into"), &[dir.join("into/a.txt")], false);
    assert!(dir.join("into/a.txt").exists() && s.error.is_none());
    // ⌥ copies.
    ops::drop_into(&mut s, &dir, &[dir.join("into/a.txt")], true);
    assert!(dir.join("a.txt").exists() && dir.join("into/a.txt").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rename_keeps_the_selection_and_refuses_clashes() {
    let dir = scratch("rename");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    std::fs::write(dir.join("b.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    ops::begin_rename(&mut s, "a.txt".into());
    // The stem is selected so typing keeps the extension.
    assert_eq!((s.rename.anchor, s.rename.focus), (0, 1));
    s.rename = rune::TextEdit::new("b.txt");
    ops::commit_rename(&mut s);
    assert!(s.error.as_deref().is_some_and(|e| e.contains("이미 사용 중")));
    ops::begin_rename(&mut s, "a.txt".into());
    s.rename = rune::TextEdit::new("c.txt");
    ops::commit_rename(&mut s);
    assert_eq!(names(&s), ["b.txt", "c.txt"]);
    assert_eq!(model::selected_ids(&s), ["c.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn icon_view_picks_extend_toggle_and_survive_a_switch() {
    let dir = scratch("picks");
    for n in ["a", "b", "c", "d"] {
        std::fs::write(dir.join(n), "").unwrap();
    }
    let mut s = app_in(&dir);
    model::set_view(&mut s, View::Icons);
    model::pick(&mut s, "a", rune::Mods::default());
    model::pick(&mut s, "c", rune::Mods { shift: true, ..Default::default() });
    assert_eq!(model::selected_ids(&s), ["a", "b", "c"]);
    model::pick(&mut s, "b", rune::Mods { cmd: true, ..Default::default() });
    assert_eq!(model::selected_ids(&s), ["a", "c"]);
    model::set_view(&mut s, View::List);
    // Every view shares one selection now, so the list keeps both.
    assert_eq!(model::selected_ids(&s), ["a", "c"]);
    // The gallery always shows an item.
    model::clear_selection(&mut s);
    model::set_view(&mut s, View::Gallery);
    assert_eq!(model::selected_ids(&s), ["a"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn go_to_folder_expands_the_home_folder() {
    let s = model::init();
    assert_eq!(model::resolve(&s, "~"), model::home());
    assert_eq!(model::resolve(&s, "~/Documents"), model::home().join("Documents"));
    assert_eq!(model::resolve(&s, "/tmp"), PathBuf::from("/tmp"));
}

#[test]
fn every_menu_shortcut_parses() {
    assert!(commands::menu_bar().len() > 20);
    for keys in ["cmd+shift+n", "cmd+backspace", "cmd+alt+v", "cmd+shift+period", "cmd+bracketleft", "cmd+up", "cmd+alt+l", "cmd+ctrl+s"] {
        assert!(keys.parse::<rune::Shortcut>().is_ok(), "{keys}");
    }
}

#[test]
fn column_view_opens_a_column_per_picked_folder() {
    let dir = scratch("cols");
    std::fs::create_dir_all(dir.join("a/b")).unwrap();
    std::fs::write(dir.join("a/b/deep.txt"), "x").unwrap();
    std::fs::write(dir.join("a/one.txt"), "x").unwrap();
    std::fs::write(dir.join("top.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::set_view(&mut s, View::Columns);
    assert_eq!(s.cols.len(), 1);
    model::col_pick(&mut s, 0, "a");
    assert_eq!(s.cols.len(), 2);
    assert_eq!(s.cols[1].entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["b", "one.txt"]);
    model::col_pick(&mut s, 1, "b");
    model::col_pick(&mut s, 2, "deep.txt");
    // Ids are relative to the window's folder, so every command finds the file.
    assert_eq!(model::selected(&s), [dir.join("a/b/deep.txt")]);
    assert_eq!(model::active_col(&s), 2);
    // Picking higher up closes the columns to its right.
    model::col_pick(&mut s, 0, "top.txt");
    assert_eq!(s.cols.len(), 1);
    // A rename in a deep column keeps working.
    model::col_pick(&mut s, 0, "a");
    model::col_pick(&mut s, 1, "one.txt");
    ops::begin_rename(&mut s, "a/one.txt".into());
    s.rename = rune::TextEdit::new("two.txt");
    ops::commit_rename(&mut s);
    assert!(dir.join("a/two.txt").exists());
    assert_eq!(s.cols[1].entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["b", "two.txt"]);
    assert_eq!(model::selected_ids(&s), ["a/two.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn delete_immediately_asks_then_removes_for_good() {
    let dir = scratch("delete");
    std::fs::create_dir_all(dir.join("folder/inner")).unwrap();
    std::fs::write(dir.join("file.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::select_all(&mut s);
    ops::ask_delete(&mut s);
    assert_eq!(s.confirm_delete.as_ref().map(Vec::len), Some(2));
    assert!(dir.join("file.txt").exists(), "nothing goes before the answer");
    ops::delete_now(&mut s);
    assert!(!dir.join("file.txt").exists() && !dir.join("folder").exists());
    assert!(s.rows.is_empty() && s.error.is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}
