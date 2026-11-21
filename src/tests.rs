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

fn ids(s: &App) -> Vec<String> {
    s.rows.iter().map(|&i| s.all[i].id.clone()).collect()
}

#[test]
fn an_expanded_folder_lists_its_items_under_it_in_order() {
    let dir = scratch("outline");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    std::fs::write(dir.join("a/z.txt"), "x").unwrap();
    std::fs::write(dir.join("a/y.txt"), "x").unwrap();
    std::fs::write(dir.join("c.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::toggle_expanded(&mut s, "a");
    assert_eq!(ids(&s), ["a", "a/y.txt", "a/z.txt", "b", "c.txt"]);
    // Other views show the folder's own items only, and the list gets its outline back.
    model::set_view(&mut s, View::Icons);
    assert_eq!(ids(&s), ["a", "b", "c.txt"]);
    model::set_view(&mut s, View::List);
    assert_eq!(ids(&s), ["a", "a/y.txt", "a/z.txt", "b", "c.txt"]);
    // A reload keeps it open; closing it takes its items away.
    model::load(&mut s);
    assert_eq!(ids(&s).len(), 5);
    model::toggle_expanded(&mut s, "a");
    assert_eq!(ids(&s), ["a", "b", "c.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn renaming_an_item_of_an_expanded_folder_keeps_it_in_that_folder() {
    let dir = scratch("nested-rename");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::write(dir.join("a/old.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::toggle_expanded(&mut s, "a");
    ops::begin_rename(&mut s, "a/old.txt".into());
    s.rename = rune::TextEdit::new("new.txt");
    ops::commit_rename(&mut s);
    assert!(dir.join("a/new.txt").exists());
    assert!(!dir.join("new.txt").exists());
    assert_eq!(model::selected_ids(&s), ["a/new.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn grouping_by_kind_starts_a_group_at_each_kind() {
    let dir = scratch("group");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("b.md"), "x").unwrap();
    std::fs::write(dir.join("a.md"), "x").unwrap();
    let mut s = app_in(&dir);
    s.group = model::Group::Kind;
    model::refresh(&mut s);
    assert_eq!(ids(&s), ["a.md", "b.md", "sub"]);
    assert_eq!(s.groups, [(0, "Markdown 문서".to_owned()), (2, "폴더".to_owned())]);
    // The icon view groups too, each group starting its own line; the gallery doesn't.
    model::set_view(&mut s, View::Icons);
    assert_eq!(s.groups.len(), 2);
    use files::IconLine::{Items, Title};
    assert_eq!(files::icon_lines(&s, 1), [Title(0), Items(0, 1), Items(1, 2), Title(1), Items(2, 3)]);
    assert_eq!(files::icon_lines(&s, 5), [Title(0), Items(0, 2), Title(1), Items(2, 3)]);
    model::set_view(&mut s, View::Gallery);
    assert!(s.groups.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn modified_groups_follow_finders_buckets() {
    let now = format::now();
    let day = 86_400;
    assert_eq!(format::day_group(now, now).1, "오늘");
    assert_eq!(format::day_group(now - 3 * day, now).1, "지난 7일");
    assert_eq!(format::day_group(now - 20 * day, now).1, "지난 30일");
    let old = format::day_group(now - 800 * day, now);
    assert!(old.1.ends_with('년') && old.0 > format::day_group(now - 20 * day, now).0);
}

#[test]
fn folder_sizes_arrive_once_counting_is_on() {
    let dir = scratch("sizes");
    std::fs::create_dir_all(dir.join("f/g")).unwrap();
    std::fs::write(dir.join("f/one"), [0u8; 1000]).unwrap();
    std::fs::write(dir.join("f/g/two"), [0u8; 500]).unwrap();
    let mut s = app_in(&dir);
    let f = s.all.iter().find(|e| e.id == "f").unwrap().clone();
    assert_eq!(model::size_of(&s, &f), None);
    model::toggle_sizes(&mut s);
    let start = std::time::Instant::now();
    while !s.sizes.contains_key(&dir.join("f")) && start.elapsed().as_secs() < 10 {
        model::take_sizes(&mut s);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(model::size_of(&s, &f), Some(1500));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn tags_go_on_and_come_off_the_file() {
    let dir = scratch("tags");
    std::fs::write(dir.join("t.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::select_only(&mut s, "t.txt".into());
    ops::toggle_tag(&mut s, "빨간색");
    assert_eq!(s.error, None);
    assert_eq!(mac::tags(&dir.join("t.txt")), ["빨간색"]);
    // Read back from disk, not just the listing's copy.
    model::load(&mut s);
    assert_eq!(model::entry(&s, "t.txt").unwrap().tags, ["빨간색"]);
    ops::toggle_tag(&mut s, "빨간색");
    assert!(mac::tags(&dir.join("t.txt")).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sidebar_favorites_are_added_saved_and_removed() {
    let dir = scratch("favorites");
    std::fs::create_dir_all(dir.join("proj")).unwrap();
    let file = dir.join("conf/favorites");
    let mut s = app_in(&dir);
    s.favorites_file = Some(file.clone());
    s.favorites = model::load_favorites(Some(&file));
    assert_eq!(s.favorites, model::default_favorites());
    model::select_only(&mut s, "proj".into());
    model::add_to_sidebar(&mut s);
    model::add_to_sidebar(&mut s);
    let saved = model::load_favorites(Some(&file));
    assert_eq!(saved.iter().filter(|p| **p == dir.join("proj")).count(), 1);
    model::remove_from_sidebar(&mut s, &dir.join("proj"));
    assert!(!model::load_favorites(Some(&file)).contains(&dir.join("proj")));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn duplicating_inside_an_expanded_folder_selects_the_copy_there() {
    let dir = scratch("nested-dup");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::write(dir.join("a/x.txt"), "x").unwrap();
    let mut s = app_in(&dir);
    model::toggle_expanded(&mut s, "a");
    model::select_only(&mut s, "a/x.txt".into());
    ops::duplicate(&mut s);
    assert!(dir.join("a/x 사본.txt").exists());
    assert_eq!(model::selected_ids(&s), ["a/x 사본.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn favorites_move_by_drag_and_take_dropped_folders_only() {
    let dir = scratch("fav-move");
    std::fs::create_dir_all(dir.join("x")).unwrap();
    std::fs::write(dir.join("file.txt"), "x").unwrap();
    let file = dir.join("favorites");
    let mut s = app_in(&dir);
    s.favorites_file = Some(file.clone());
    s.favorites = vec![PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c")];
    model::move_favorite(&mut s, 0, 2);
    assert_eq!(s.favorites, [PathBuf::from("/b"), PathBuf::from("/c"), PathBuf::from("/a")]);
    model::move_favorite(&mut s, 2, 0);
    assert_eq!(model::load_favorites(Some(&file)), [PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c")]);
    // A drop adds the folder once and leaves the file out.
    model::add_favorites(&mut s, &[dir.join("x"), dir.join("file.txt"), dir.join("x")]);
    assert_eq!(s.favorites.last(), Some(&dir.join("x")));
    assert_eq!(s.favorites.len(), 4);
    std::fs::remove_dir_all(&dir).unwrap();
}
