//! Finder's keyboard shortcuts and its menu bar (파일, 편집, 보기, 이동), both running the same
//! commands.
use std::path::PathBuf;

use rune::MenuItem;
use rune::{Key, KeyEvent, Shortcut, Tx};

use crate::model::{self, App, View};
use crate::{dialogs, ops};

type Command = fn(&mut App, &mut Tx);

fn place(s: &mut App, p: PathBuf) {
    if p.is_dir() {
        model::go(s, p);
    } else {
        s.error = Some(format!("“{}” 폴더를 찾을 수 없습니다.", model::display_name(&p)));
    }
}

fn home(s: &mut App, _: &mut Tx) {
    place(s, model::home());
}
fn desktop(s: &mut App, _: &mut Tx) {
    place(s, model::home().join("Desktop"));
}
fn documents(s: &mut App, _: &mut Tx) {
    place(s, model::home().join("Documents"));
}
fn downloads(s: &mut App, _: &mut Tx) {
    place(s, model::home().join("Downloads"));
}
fn applications(s: &mut App, _: &mut Tx) {
    place(s, PathBuf::from("/Applications"));
}
fn utilities(s: &mut App, _: &mut Tx) {
    place(s, PathBuf::from("/Applications/Utilities"));
}
fn computer(s: &mut App, _: &mut Tx) {
    place(s, PathBuf::from("/"));
}
fn icloud(s: &mut App, _: &mut Tx) {
    match model::icloud() {
        Some(p) => place(s, p),
        None => s.error = Some("iCloud Drive를 사용할 수 없습니다.".into()),
    }
}
fn enclosing(s: &mut App, tx: &mut Tx) {
    model::up(s, tx);
}
fn back(s: &mut App, _: &mut Tx) {
    model::history(s, true);
}
fn forward(s: &mut App, _: &mut Tx) {
    model::history(s, false);
}
fn goto(s: &mut App, _: &mut Tx) {
    dialogs::open_goto(s);
}
fn new_folder(s: &mut App, _: &mut Tx) {
    if !model::searching(s) {
        ops::new_folder(s);
    }
}
fn open(s: &mut App, _: &mut Tx) {
    ops::open_selected(s);
}
fn info(s: &mut App, _: &mut Tx) {
    ops::get_info(s);
}
fn duplicate(s: &mut App, _: &mut Tx) {
    ops::duplicate(s);
}
fn compress(s: &mut App, _: &mut Tx) {
    ops::compress(s);
}
fn trash(s: &mut App, _: &mut Tx) {
    ops::trash(s);
}
fn quick_look(s: &mut App, _: &mut Tx) {
    ops::quick_look(s);
}
fn paste(s: &mut App, tx: &mut Tx) {
    ops::paste(s, false, tx);
}
fn move_here(s: &mut App, tx: &mut Tx) {
    ops::paste(s, true, tx);
}
fn select_all(s: &mut App, _: &mut Tx) {
    model::select_all(s);
}
fn as_icons(s: &mut App, _: &mut Tx) {
    model::set_view(s, View::Icons);
}
fn as_list(s: &mut App, _: &mut Tx) {
    model::set_view(s, View::List);
}
fn hidden(s: &mut App, _: &mut Tx) {
    ops::toggle_hidden(s);
}
fn sidebar(s: &mut App, _: &mut Tx) {
    s.sidebar.toggle();
}
fn find(s: &mut App, _: &mut Tx) {
    s.search.focused = true;
}
fn reload(s: &mut App, _: &mut Tx) {
    model::load(s);
}

/// Keys the window handles once the focused control passes them on.
pub fn key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let run: Command = match &ev.key {
        Key::Up if m.cmd => {
            return model::up(s, tx);
        }
        Key::Down if m.cmd => open,
        Key::Backspace if m.cmd && !m.shift => trash,
        Key::Enter if !m.cmd => return ops::begin_rename_selected(s),
        Key::Escape if s.menu.is_some() => {
            s.menu = None;
            return true;
        }
        Key::Text(t) if t == " " && !m.cmd && !model::typing(s) => quick_look,
        Key::Text(t) if !m.cmd && !m.ctrl => return model::type_select(s, t, tx),
        Key::Command(c) => match (*c, m.shift, m.alt) {
            ('[', false, false) => back,
            (']', false, false) => forward,
            ('o', false, false) => open,
            ('i', false, false) => info,
            ('d', false, false) => duplicate,
            ('r', false, false) if model::searching(s) => ops::show_in_folder,
            ('r', false, false) => reload,
            ('c', false, false) => ops::copy,
            ('c', false, true) => ops::copy_path,
            ('v', false, alt) => {
                if alt { move_here } else { paste }
            }
            ('a', false, false) => select_all,
            ('f', false, false) => find,
            ('1', false, false) => as_icons,
            ('2', false, false) => as_list,
            ('n', true, false) => new_folder,
            ('.', true, false) => hidden,
            ('g', true, false) => goto,
            ('h', true, false) => home,
            ('d', true, false) => desktop,
            ('o', true, false) => documents,
            ('a', true, false) => applications,
            ('u', true, false) => utilities,
            ('c', true, false) => computer,
            ('i', true, false) => icloud,
            ('l', false, true) => downloads,
            ('s', false, false) if m.ctrl => sidebar,
            _ => return false,
        },
        _ => return false,
    };
    // A typed name stops collecting once a command runs.
    s.typed.1 = None;
    run(s, tx);
    true
}

/// Finder's menus. Each item lets the window's keys go first, so a focused field keeps its
/// own ⌘C and ⌘V, and `key` runs the same command when the shortcut is typed.
pub fn menu_bar() -> Vec<MenuItem<App>> {
    let item = |menu: &str, title: &str, keys: &str, run: Command| {
        let mut i = MenuItem::new(title, run).id(format!("{menu}:{title}")).menu(menu).keys_first();
        if let Ok(sc) = keys.parse::<Shortcut>() {
            i = i.key_equivalent(sc);
        }
        i
    };
    let sep = |menu: &str| MenuItem::separator().menu(menu);
    vec![
        item("파일", "새로운 폴더", "cmd+shift+n", new_folder),
        item("파일", "열기", "cmd+o", open),
        sep("파일"),
        item("파일", "정보 가져오기", "cmd+i", info),
        item("파일", "압축", "", compress),
        item("파일", "복제", "cmd+d", duplicate),
        item("파일", "훑어보기", "cmd+y", quick_look),
        sep("파일"),
        item("파일", "휴지통으로 이동", "cmd+backspace", trash),
        item("편집", "복사", "cmd+c", ops::copy),
        item("편집", "붙여넣기", "cmd+v", paste),
        item("편집", "항목 이동", "cmd+alt+v", move_here),
        item("편집", "모두 선택", "cmd+a", select_all),
        item("편집", "경로 이름 복사", "cmd+alt+c", ops::copy_path),
        item("보기", "아이콘으로", "cmd+1", as_icons),
        item("보기", "목록으로", "cmd+2", as_list),
        sep("보기"),
        item("보기", "숨김 파일 보기/가리기", "cmd+shift+period", hidden),
        item("보기", "사이드바 보기/가리기", "cmd+ctrl+s", sidebar),
        item("이동", "뒤로", "cmd+bracketleft", back),
        item("이동", "앞으로", "cmd+bracketright", forward),
        item("이동", "상위 폴더", "cmd+up", enclosing),
        sep("이동"),
        item("이동", "컴퓨터", "cmd+shift+c", computer),
        item("이동", "홈", "cmd+shift+h", home),
        item("이동", "데스크탑", "cmd+shift+d", desktop),
        item("이동", "문서", "cmd+shift+o", documents),
        item("이동", "다운로드", "cmd+alt+l", downloads),
        item("이동", "응용 프로그램", "cmd+shift+a", applications),
        item("이동", "유틸리티", "cmd+shift+u", utilities),
        item("이동", "iCloud Drive", "cmd+shift+i", icloud),
        sep("이동"),
        item("이동", "폴더로 이동…", "cmd+shift+g", goto),
    ]
}
