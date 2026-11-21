//! Around the files: the sidebar, toolbar, search scope bar, path bar and status bar.
use std::path::{Path, PathBuf};

use rune::{El, Icon, el, icon, row, text};
use rune_kit::shell::{Sidebar, SidebarRow, SidebarSection, Toolbar};
use rune_kit::{IconButton, IconButtonVariant, Kit, Menu, MenuItem, TextInput, context_menu, glyphs};

use crate::mac::{self, Share};
use crate::model::{self, App, Group, Popup, Scope, View};
use crate::{close_menus, file_menu, format, icons, ops};

fn place(s: &App, k: &Kit, label: &str, glyph: &'static Icon, p: PathBuf) -> El {
    place_with(s, k, label, glyph, p, |r| r)
}

/// A sidebar place, with `more` adding to its row before it's built (a favorite's reorder).
fn place_with(s: &App, k: &Kit, label: &str, glyph: &'static Icon, p: PathBuf, more: impl FnOnce(SidebarRow) -> SidebarRow) -> El {
    let (to, target) = (p.clone(), p.clone());
    let r = SidebarRow::new(label.to_owned())
        .icon(glyph)
        .height(28.0)
        .selected(s.dir == p && !model::searching(s))
        .on_select(move |s: &mut App, _| {
            // A favorite dragged to a new place stays where the window is, as in Finder.
            if !std::mem::take(&mut s.fav_moved) {
                model::go(s, to.clone());
            }
        });
    more(r)
        .build(k)
        .key(format!("place:{}", p.display()))
        // Dropping files on a sidebar folder moves them there, as in Finder.
        .on_drop_files(move |s: &mut App, files, tx| ops::drop_into(s, &target, files, tx.mods().alt))
}

fn volume(s: &App, k: &Kit, rk: &Kit, v: &Path) -> El {
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
        .icon(&icons::EXTERNAL_DISK)
        .height(28.0)
        .selected(s.dir == v && !model::searching(s))
        .trailing(eject)
        .on_select(move |s: &mut App, _| model::go(s, to.clone()))
        .build(rk)
        .key(format!("place:{}", v.display()))
}

pub fn sidebar(s: &App, k: &Kit) -> El {
    let h = model::home();
    // Finder's sidebar rows: names in full-strength text and icons in the accent colour,
    // selected or not. Section titles and eject buttons keep the plain kit's grey.
    let mut rk = k.clone();
    rk.c.text_muted = k.c.text_base;
    rk.c.icon_muted = k.c.icon_accent;
    rk.c.icon_base = k.c.icon_accent;
    // Locations show their disks in grey, as Finder's do.
    let mut grey = rk.clone();
    grey.c.icon_muted = k.c.icon_muted;
    grey.c.icon_base = k.c.icon_base;
    let (k, plain) = (&rk, k);
    // The user's favorites: Finder's own folders keep their names and symbols.
    let count = s.favorites.len();
    let favorite = |(i, p): (usize, &PathBuf)| -> El {
        let (label, glyph): (String, &'static Icon) = if *p == Path::new("/Applications") {
            ("응용 프로그램".into(), &icons::APPS)
        } else if *p == h.join("Desktop") {
            ("데스크탑".into(), &icons::DESKTOP)
        } else if *p == h.join("Documents") {
            ("문서".into(), &icons::DOCS)
        } else if *p == h.join("Downloads") {
            ("다운로드".into(), &icons::DOWNLOADS)
        } else if *p == h {
            (model::display_name(&h), &icons::HOUSE)
        } else {
            (model::display_name(p), &icons::SIDEBAR_FOLDER)
        };
        let target = p.clone();
        // Dragging a favorite up or down puts it in a new place, as in Finder.
        let reorder = |r: SidebarRow| r.reorder(&s.fav_reorder, i, count, |s: &mut App| &mut s.fav_reorder, model::move_favorite);
        place_with(s, k, &label, glyph, p.clone(), reorder).on_secondary_press(move |s: &mut App, at, _| {
            close_menus(s);
            s.side_menu = Some((at, target.clone()));
        })
    };
    let favorites = SidebarSection::new("즐겨찾기").rows(s.favorites.iter().enumerate().map(favorite).collect::<Vec<_>>());
    let mut bar = Sidebar::new().label("사이드바").section(favorites);
    if let Some(cloud) = model::icloud() {
        bar = bar.section(SidebarSection::new("iCloud").row(place(s, k, "iCloud Drive", &icons::CLOUD, cloud)));
    }
    let mut places = SidebarSection::new("위치").row(place(s, &grey, "Macintosh HD", &icons::DISK, PathBuf::from("/")));
    // "/Volumes/Macintosh HD" is a link back to "/".
    for v in s.volumes.iter().filter(|v| !std::fs::symlink_metadata(v).is_ok_and(|m| m.file_type().is_symlink())) {
        places = places.row(volume(s, plain, &grey, v));
    }
    // Folders dropped on the sidebar outside a place join 즐겨찾기; dropped on a place, they
    // move into it, as in Finder.
    bar.section(places).build(plain).on_drop_files(|s: &mut App, files, _| model::add_favorites(s, files))
}

/// A macOS 26 toolbar group: controls on one raised, rounded capsule.
fn capsule(k: &Kit, key: &str, e: El) -> El {
    let fill = if k.dark { k.c.layer_02 } else { k.c.bg_base };
    k.elevate(e.key(key.to_owned()).h(SEG_H + 8.0).px(4.0).center_y().radius(SEG_H / 2.0 + 4.0).fill(fill).border(0.5, k.c.border_base), &k.elevation.raised)
}

/// Inside a capsule: as tall as its inside and rounded to match, so a hover or a pick is a
/// pill that follows the capsule's curve, as Finder's toolbar draws them.
const SEG_H: f32 = 28.0;
const SEG_W: f32 = 40.0;
/// The share, tag and … buttons: larger glyphs, closer together, as Finder's are.
const ACTION_W: f32 = 35.0;
const ACTION_GLYPH: f32 = 21.0;

fn picked(k: &Kit) -> rune::Color {
    k.alpha(k.c.text_base, if k.dark { 0.14 } else { 0.08 })
}

/// A capsule's control: `glyph` centred in a pill `w` wide, filled while `on`.
fn seg(k: &Kit, key: &str, label: &str, w: f32, on: bool, enabled: bool, content: El) -> El {
    let mut b = row().key(key.to_owned()).role("button").label(label.to_owned()).w(w).h(SEG_H).gap(3.0).center().radius(SEG_H / 2.0).disabled(!enabled);
    if enabled {
        b = b.hover(k.c.hover).cursor(rune::Cursor::Pointer);
    }
    if on {
        b = b.fill(picked(k));
    }
    b.child(content)
}

fn glyph(k: &Kit, g: &'static Icon, enabled: bool) -> El {
    icon(g, 18.0).color(if enabled { k.c.icon_base } else { k.c.icon_faint })
}

/// A hairline between a capsule's controls.
fn divider(k: &Kit) -> El {
    el().w(1.0).h(18.0).mx(2.0).fill(k.c.border_weak).a11y_hidden()
}

/// Finder's view switcher: icons (⌘1), list (⌘2), columns (⌘3) and gallery (⌘4).
fn view_switch(s: &App, k: &Kit) -> El {
    let choice = |g: &'static Icon, label: &str, v: View| {
        let on = s.view == v;
        seg(k, label, label, SEG_W, on, true, glyph(k, g, true)).role("radio").checked(on).on_press(move |s: &mut App| model::set_view(s, v))
    };
    capsule(
        k,
        "views",
        row()
            .role("radiogroup")
            .label("보기")
            .center_y()
            .child(choice(&icons::VIEW_ICONS, "아이콘", View::Icons))
            .child(choice(&icons::VIEW_LIST, "목록", View::List))
            .child(choice(&icons::VIEW_COLUMNS, "계층", View::Columns))
            .child(divider(k))
            .child(choice(&icons::VIEW_GALLERY, "갤러리", View::Gallery)),
    )
}

/// A toolbar menu: `trigger` opens it under itself, and opening one closes any other.
fn popup(s: &App, k: &Kit, layer: &mut Vec<El>, which: Popup, (label, w, content): (&str, f32, El), items: impl FnOnce(Menu) -> Menu) -> El {
    let open = s.popup == Some(which);
    let button = seg(k, &format!("{which:?}").to_lowercase(), label, w, open, true, content).expanded(open).on_press(move |s: &mut App| {
        let open = s.popup == Some(which);
        close_menus(s);
        s.popup = (!open).then_some(which);
    });
    items(Menu::new(format!("popup-{which:?}").to_lowercase()).trigger(button).open(s.popup == Some(which)))
        .on_close(|s: &mut App| s.popup = None)
        .build(k, layer)
}

/// Finder's group and sort menu: 그룹 사용 (the list's), 정렬 기준, and 모든 크기 계산.
fn group_items(s: &App, menu: Menu) -> Menu {
    let list = model::groups_shown(s) && !model::searching(s);
    let group = |label: &str, g: Group| {
        MenuItem::new(label.to_owned()).checked(s.group == g).disabled(!list).on_select(move |s: &mut App| {
            s.popup = None;
            s.group = g;
            model::refresh(s);
        })
    };
    let sort = |label: &str, id: &'static str| {
        MenuItem::new(label.to_owned()).checked(s.sort.0 == id).on_select(move |s: &mut App| {
            s.popup = None;
            if s.sort.0 != id {
                let dir = if id == "modified" || id == "size" { rune_kit::SortDir::Descending } else { rune_kit::SortDir::Ascending };
                s.sort = (id.to_owned(), dir);
                model::refresh(s);
            }
        })
    };
    menu.min_width(200.0)
        .group_label("그룹 사용")
        .item(group("없음", Group::None))
        .item(group("종류", Group::Kind))
        .item(group("수정일", Group::Modified))
        .item(group("크기", Group::Size))
        .separator()
        .group_label("정렬 기준")
        .item(sort("이름", "name"))
        .item(sort("종류", "kind"))
        .item(sort("수정일", "modified"))
        .item(sort("크기", "size"))
        .separator()
        .item(MenuItem::new("모든 크기 계산").checked(s.calc_sizes).on_select(|s: &mut App| {
            s.popup = None;
            model::toggle_sizes(s);
        }))
}

fn share_items(menu: Menu) -> Menu {
    let to = |label: &str, t: Share| {
        MenuItem::new(label.to_owned()).on_select(move |s: &mut App| {
            s.popup = None;
            ops::share(s, t);
        })
    };
    menu.min_width(180.0).item(to("AirDrop", Share::AirDrop)).item(to("메일", Share::Mail)).item(to("메시지", Share::Messages))
}

/// A tag's colour dot, Finder's palette; a tag of the user's own is a grey ring.
pub fn tag_dot(k: &Kit, tag: &str, size: f32) -> El {
    const COLORS: [&str; 7] = ["#FF453A", "#FF9F0A", "#FFD60A", "#32D74B", "#0A84FF", "#BF5AF2", "#8E8E93"];
    let dot = el().w(size).h(size).radius(size / 2.0);
    match mac::tag_color(tag) {
        Some(i) => {
            dot.fill(rune::color::hex(COLORS[i]).unwrap_or(k.c.icon_muted)).border(1.0, k.c.bg_base)
        }
        None => dot.border(1.0, k.c.icon_muted),
    }
}

fn tag_items(s: &App, k: &Kit, menu: Menu) -> Menu {
    let ids = model::selected_ids(s);
    let mut menu = menu.min_width(180.0);
    for tag in mac::TAGS {
        let on = !ids.is_empty() && ids.iter().all(|id| model::entry(s, id).is_some_and(|e| e.tags.iter().any(|t| t == tag)));
        menu = menu.item(MenuItem::new(tag.to_owned()).lead(tag_dot(k, tag, 12.0)).checked(on).disabled(ids.is_empty()).on_select(move |s: &mut App| {
            s.popup = None;
            ops::toggle_tag(s, tag);
        }));
    }
    menu
}

pub fn toolbar(s: &App, k: &Kit, layer: &mut Vec<El>) -> Toolbar {
    let nav = |g: &'static Icon, label: &str, on: bool, back: bool| {
        let b = seg(k, label, label, SEG_W, false, on, glyph(k, g, on));
        if on { b.on_press(move |s: &mut App| model::history(s, back)) } else { b }
    };
    // Finder's search is a round button until it's used; ⌘F opens it too. It stays a field
    // while it has focus or text.
    let search = if s.search.focused || !s.search.text.is_empty() {
        TextInput::new(&s.search)
            .label("검색")
            .placeholder("검색")
            .leading_icon(&glyphs::SEARCH)
            .width(Some(180.0))
            .clear_button()
            .on_edit(|s: &mut App| &mut s.search)
            .on_change(|s: &mut App, _| model::request_search(s))
            .build(k)
            .key("search")
    } else {
        capsule(k, "search-button", row().child(seg(k, "search", "검색", SEG_H, false, true, glyph(k, &icons::SEARCH, true)).on_press(|s: &mut App| s.search.focused = true)))
    };
    let title = if model::searching(s) { format!("검색: “{}”", s.search.text.trim()) } else { model::display_name(&s.dir) };

    // The title sits past the back and forward buttons with the folder's icon, as in Finder.
    let heading = row()
        .key("title")
        .gap(6.0)
        .center_y()
        .shrink()
        .min_w(0.0)
        .child(crate::files::path_icon(s, &s.dir, 20.0))
        .child(text(title).font(k.ty.body_medium).color(k.c.text_base).truncate().shrink().min_w(0.0));
    Toolbar::new()
        .leading(capsule(
            k,
            "nav",
            row().center_y().child(nav(&icons::BACK, "뒤로", !s.back.is_empty(), true)).child(divider(k)).child(nav(&icons::FORWARD, "앞으로", !s.fwd.is_empty(), false)),
        ))
        .leading(heading.ml(6.0))
        .trailing({
            // The group button carries a small chevron after its grid, as Finder's does.
            // Finder's measures: 45.5 pt across the capsule, 5.75 pt between grid and chevron.
            let grid = row().gap(5.75).center_y().child(glyph(k, &icons::GROUP, true)).child(icon(&icons::GROUP_CHEVRON, 8.0).color(k.c.icon_base));
            let group = popup(s, k, layer, Popup::Group, ("그룹 및 정렬", 37.5, grid), |m| group_items(s, m));
            // Finder's measures: 21 pt glyphs, 37 pt apart, in a capsule about 111 pt across.
            let action = |g: &'static Icon| icon(g, ACTION_GLYPH).color(k.c.icon_base);
            let share = popup(s, k, layer, Popup::Share, ("공유", ACTION_W, action(&icons::SHARE)), share_items);
            let tags = popup(s, k, layer, Popup::Tags, ("태그", ACTION_W, action(&icons::TAG)), |m| tag_items(s, k, m));
            let more = popup(s, k, layer, Popup::More, ("동작", ACTION_W, action(&icons::MORE)), |m| file_menu(s, m.min_width(260.0)));
            row()
                .gap(10.0)
                .center_y()
                .child(view_switch(s, k))
                .child(capsule(k, "group-capsule", row().child(group)))
                .child(capsule(k, "actions", row().gap(2.0).child(share).child(tags).child(more)).px(2.0))
                .child(search)
        })
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
    // The bar ends at the one selected item, as Finder's does; searching, it shows where the
    // selected result lives.
    let picked = model::selected(s);
    let shown = match picked.as_slice() {
        [one] => Some(one.clone()),
        _ if model::searching(s) => model::focus_id(s).map(|id| s.dir.join(id)),
        _ => Some(s.dir.clone()),
    };
    let Some(shown) = shown else { return bar };
    let crumbs: Vec<&Path> = shown.ancestors().collect();
    for (i, p) in crumbs.iter().rev().enumerate() {
        if i > 0 {
            bar = bar.child(icon(&glyphs::CARET_RIGHT, 10.0).color(k.c.icon_faint));
        }
        let to = p.to_path_buf();
        let glyph = crate::files::path_icon(s, p, 16.0);
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

/// A favorite's context menu in the sidebar.
pub fn side_menu(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let at = s.side_menu.as_ref().map(|(p, _)| *p);
    context_menu("side-menu", el(), at)
        .item(MenuItem::new("사이드바에서 제거").on_select(|s: &mut App| {
            if let Some((_, p)) = s.side_menu.take() {
                model::remove_from_sidebar(s, &p);
            }
        }))
        .on_close(|s: &mut App| s.side_menu = None)
        .build(k, layer)
}
