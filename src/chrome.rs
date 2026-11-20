//! Around the files: the sidebar, toolbar, search scope bar, path bar and status bar.
use std::path::{Path, PathBuf};

use rune::{El, Icon, icon, row, text};
use rune_kit::shell::{Sidebar, SidebarRow, SidebarSection, Toolbar};
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput, glyphs};

use crate::model::{self, App, Scope, View};
use crate::{format, icons, ops};

fn place(s: &App, k: &Kit, label: &str, glyph: &'static Icon, p: PathBuf) -> El {
    let (to, target) = (p.clone(), p.clone());
    SidebarRow::new(label.to_owned())
        .icon(glyph)
        .height(28.0)
        .selected(s.dir == p && !model::searching(s))
        .on_select(move |s: &mut App, _| model::go(s, to.clone()))
        .build(k)
        .key(format!("place:{}", p.display()))
        // Dropping files on a sidebar folder moves them there, as in Finder.
        .on_drop_files(move |s: &mut App, files, tx| ops::drop_into(s, &target, files, tx.mods().alt))
}

fn volume(s: &App, k: &Kit, v: &Path) -> El {
    let target = v.to_path_buf();
    let eject = IconButton::new(&icons::EJECT)
        .variant(IconButtonVariant::GhostMuted)
        .label(format!("{} 추출", model::display_name(v)))
        .on_press(move |s: &mut App| ops::eject(s, &target))
        .build(k)
        .key("eject")
        .size(18.0, 18.0);
    let (to, label) = (v.to_path_buf(), model::display_name(v));
    SidebarRow::new(label)
        .icon(&icons::DISK)
        .height(28.0)
        .selected(s.dir == v && !model::searching(s))
        .trailing(eject)
        .on_select(move |s: &mut App, _| model::go(s, to.clone()))
        .build(k)
        .key(format!("place:{}", v.display()))
}

pub fn sidebar(s: &App, k: &Kit) -> El {
    let h = model::home();
    // Finder's default favorites, in its order.
    let favorites = SidebarSection::new("즐겨찾기").rows([
        place(s, k, "응용 프로그램", &icons::APPS, PathBuf::from("/Applications")),
        place(s, k, "데스크탑", &icons::DESKTOP, h.join("Desktop")),
        place(s, k, "문서", &icons::DOCS, h.join("Documents")),
        place(s, k, "다운로드", &icons::DOWNLOADS, h.join("Downloads")),
        place(s, k, &model::display_name(&h), &icons::HOUSE, h.clone()),
    ]);
    let mut bar = Sidebar::new().label("사이드바").section(favorites);
    if let Some(cloud) = model::icloud() {
        bar = bar.section(SidebarSection::new("iCloud").row(place(s, k, "iCloud Drive", &icons::CLOUD, cloud)));
    }
    let mut places = SidebarSection::new("위치").row(place(s, k, "Macintosh HD", &icons::DISK, PathBuf::from("/")));
    // "/Volumes/Macintosh HD" is a link back to "/".
    for v in s.volumes.iter().filter(|v| !std::fs::symlink_metadata(v).is_ok_and(|m| m.file_type().is_symlink())) {
        places = places.row(volume(s, k, v));
    }
    places = places.row(place(s, k, "휴지통", &icons::TRASH, h.join(".Trash")));
    bar.section(places).build(k)
}

/// Finder's view switcher: icons (⌘1), list (⌘2), columns (⌘3) and gallery (⌘4).
fn view_switch(s: &App, k: &Kit) -> El {
    let seg = |glyph: &'static Icon, label: &str, v: View| {
        let on = s.view == v;
        let mut b = row()
            .key(label.to_owned())
            .role("radio")
            .label(label.to_owned())
            .checked(on)
            .w(30.0)
            .h(24.0)
            .center()
            .radius(5.0)
            .hover(k.c.hover)
            .on_press(move |s: &mut App| model::set_view(s, v))
            .child(icon(glyph, 16.0).color(if on { k.c.icon_base } else { k.c.icon_muted }));
        if on {
            b = b.fill(k.alpha(k.c.text_base, if k.dark { 0.14 } else { 0.08 }));
        }
        b
    };
    row()
        .key("views")
        .role("radiogroup")
        .label("보기")
        .gap(1.0)
        .child(seg(&icons::VIEW_ICONS, "아이콘", View::Icons))
        .child(seg(&icons::VIEW_LIST, "목록", View::List))
        .child(seg(&icons::VIEW_COLUMNS, "계층", View::Columns))
        .child(seg(&icons::VIEW_GALLERY, "갤러리", View::Gallery))
}

pub fn toolbar(s: &App, k: &Kit) -> Toolbar {
    let nav = |glyph: &'static Icon, label: &str, on: bool, back: bool| {
        IconButton::new(glyph)
            .variant(IconButtonVariant::GhostMuted)
            .label(label.to_owned())
            .disabled(!on)
            .on_press(move |s: &mut App| model::history(s, back))
            .build(k)
            .key(label.to_owned())
    };
    let search = TextInput::new(&s.search)
        .label("검색")
        .placeholder("검색")
        .leading_icon(&glyphs::SEARCH)
        .width(Some(180.0))
        .clear_button()
        .on_edit(|s: &mut App| &mut s.search)
        .on_change(|s: &mut App, _| model::request_search(s))
        .build(k)
        .key("search");
    let title = if model::searching(s) { format!("검색: “{}”", s.search.text.trim()) } else { model::display_name(&s.dir) };
    Toolbar::new()
        .leading(row().gap(2.0).child(nav(&icons::BACK, "뒤로", !s.back.is_empty(), true)).child(nav(&icons::FORWARD, "앞으로", !s.fwd.is_empty(), false)))
        .title(title)
        .trailing(row().gap(10.0).center_y().child(view_switch(s, k)).child(search))
}

/// While searching: where to look, "이 Mac" or the folder the search started in.
pub fn scope_bar(s: &App, k: &Kit) -> El {
    let chip = |label: String, scope: Scope| {
        let on = s.scope == scope;
        let mut c = row()
            .key(format!("scope:{scope:?}"))
            .role("radio")
            .checked(on)
            .label(label.clone())
            .h(22.0)
            .px(8.0)
            .center()
            .radius(5.0)
            .hover(k.c.hover)
            .on_press(move |s: &mut App| {
                s.scope = scope;
                model::request_search(s);
            })
            .child(text(label).font(k.ty.text_small_medium).color(if on { k.c.text_base } else { k.c.text_muted }));
        if on {
            c = c.fill(k.alpha(k.c.text_base, if k.dark { 0.14 } else { 0.08 }));
        }
        c
    };
    let mut bar = row()
        .key("scope")
        .w_full()
        .h(30.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak)
        .child(text("검색:").font(k.ty.text_small).color(k.c.text_muted))
        .child(chip("이 Mac".into(), Scope::Mac))
        .child(chip(format!("“{}”", model::display_name(&s.dir)), Scope::Folder));
    if s.found.is_pending() {
        bar = bar.child(text("검색 중…").font(k.ty.text_small).color(k.c.text_faint));
    }
    bar
}

pub fn path_bar(s: &App, k: &Kit) -> El {
    let mut bar = row().key("pathbar").w_full().h(24.0).px(10.0).gap(4.0).center_y().border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak);
    // Searching, the bar shows where the selected result lives, as Finder's does.
    let shown = if model::searching(s) { model::focus_id(s).map(|id| s.dir.join(id)) } else { Some(s.dir.clone()) };
    let Some(shown) = shown else { return bar };
    let crumbs: Vec<&Path> = shown.ancestors().collect();
    for (i, p) in crumbs.iter().rev().enumerate() {
        if i > 0 {
            bar = bar.child(icon(&glyphs::CARET_RIGHT, 10.0).color(k.c.icon_faint));
        }
        let to = p.to_path_buf();
        let is_dir = i + 1 < crumbs.len() || to.is_dir();
        let look = if i == 0 { None } else { Some(format::look(&model::display_name(p), is_dir)) };
        let glyph = match look {
            None => icon(&icons::DISK, 12.0).color(k.c.icon_muted),
            Some(l) => icons::file(l, 13.0),
        };
        bar = bar.child(
            row()
                .key(format!("crumb:{i}"))
                .gap(3.0)
                .center_y()
                .child(glyph)
                .child(text(model::display_name(p)).font(k.ty.text_small).color(k.c.text_muted))
                .on_double_press(move |s: &mut App| if to.is_dir() { model::go(s, to.clone()) }),
        );
    }
    bar
}

pub fn status(s: &App, k: &Kit) -> El {
    let free = s.free.map(|b| format!(", {} 사용 가능", format::free(b))).unwrap_or_default();
    let (label, ink) = match &s.error {
        Some(e) => (e.clone(), k.c.danger.icon),
        None => match model::selected_len(s) {
            0 => (format!("{}개 항목{free}", s.rows.len()), k.c.text_muted),
            n => (format!("{}개 중 {n}개 선택됨{free}", s.rows.len()), k.c.text_muted),
        },
    };
    row().key("status").w_full().h(22.0).center().border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak).child(text(label).font(k.ty.text_small).color(ink))
}
