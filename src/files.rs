//! The folder's items in the list and icon views, and what every view's items share: their
//! icons, clicks, drags, drops, context menu and keys.
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rune::{Align, DragItem, El, Key, KeyEvent, SurfaceFit, Tx, col, el, icon, row, text};
use rune_kit::{Kit, SortDir, TextInput};

use crate::format::Look;
use crate::model::{self, App, Entry};
use crate::{format, icons, ops};

// --- shared -------------------------------------------------------------------

/// The item's icon: the system's own (Finder's icon for that file, or a thumbnail of an
/// image when it's drawn large) once decoded, and a drawn stand-in until then or off macOS.
pub fn item_icon(s: &App, e: &Entry, path: &Path, size: f32) -> El {
    if let Some(cache) = &s.images {
        let img = if e.look == Look::Image && size >= 48.0 { cache.file(path, size, 2.0) } else { cache.app_icon(path, size) };
        if img.is_ready() && img.error().is_none() {
            return el().w(size).h(size).pixels_fit(img.pixels(), SurfaceFit::Fit);
        }
    }
    icons::file(e.look, size)
}

/// What a drag of `id` carries: the whole selection when `id` is in it, else `id` alone.
fn drag_item(s: &App, id: &str) -> Option<DragItem> {
    let paths: Vec<PathBuf> = if model::is_selected(s, id) { model::selected(s) } else { vec![s.dir.join(id)] };
    let mut item = DragItem::new().allow_move().within_app();
    for p in paths {
        item = item.file(p);
    }
    Some(item)
}

/// What a press on an item does: picks it in a column, or picks it with the modifiers held.
#[derive(Clone, Copy)]
pub enum Press {
    Pick,
    Column(usize),
}

/// Makes `e` an item: click to select, double-click to open, drag out, right-click for the
/// menu, and for a folder a place to drop files.
pub fn item(e: El, s: &App, entry: &Entry, id: &str, press: Press) -> El {
    let (pick_id, open_id, drag_id, menu_id) = (id.to_owned(), id.to_owned(), id.to_owned(), id.to_owned());
    let name = entry.name.clone();
    let mut e = e
        .on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if s.renaming.as_deref() == Some(pick_id.as_str()) {
                return;
            }
            if s.renaming.is_some() {
                ops::commit_rename(s);
            }
            match press {
                Press::Pick => model::pick(s, &pick_id, tx.mods()),
                Press::Column(i) => model::col_pick(s, i, &name),
            }
        })
        .on_double_press(move |s: &mut App| ops::open(s, &open_id))
        .drag_out(move |s: &App| drag_item(s, &drag_id))
        .on_secondary_press(move |s: &mut App, p, tx| {
            // Finder acts on what was right-clicked: the selection if it's in it, else it alone.
            if !model::is_selected(s, &menu_id) {
                model::select_only(s, menu_id.clone());
            }
            s.alt = tx.mods().alt;
            s.menu = Some(p);
        });
    if entry.is_folder() {
        let (over, target) = (id.to_owned(), s.dir.join(id));
        e = e
            .on_drop_files(move |s: &mut App, files, tx| {
                s.drop_over = None;
                ops::drop_into(s, &target, files, tx.mods().alt);
            })
            .on_drop_hover(move |s: &mut App, on, _| s.drop_over = on.then(|| over.clone()));
    }
    e
}

pub fn rename_field(s: &App, k: &Kit, height: f32) -> El {
    TextInput::new(&s.rename)
        .label("이름 변경")
        .height(height)
        .on_edit(|s: &mut App| &mut s.rename)
        .on_submit(|s: &mut App, _| ops::commit_rename(s))
        .on_key(|s: &mut App, ev: &KeyEvent, _| {
            let esc = ev.key == Key::Escape;
            if esc {
                s.renaming = None;
            }
            esc
        })
        .build(k)
}

/// Keys every view's items take while it has focus: arrows move (`row` items to a line),
/// Return renames, Space opens Quick Look.
pub fn view_keys(s: &mut App, ev: &KeyEvent, tx: &mut Tx, row: isize, across: bool) -> bool {
    let m = ev.mods;
    if m.cmd || m.ctrl {
        return false;
    }
    match &ev.key {
        Key::Up => model::step(s, -row, m.shift, tx),
        Key::Down => model::step(s, row, m.shift, tx),
        Key::Left if across => model::step(s, -1, m.shift, tx),
        Key::Right if across => model::step(s, 1, m.shift, tx),
        Key::Left if model::in_columns(s) => return model::col_step(s, false, tx),
        Key::Right if model::in_columns(s) => return model::col_step(s, true, tx),
        Key::Home => model::step(s, -isize::MAX / 2, m.shift, tx),
        Key::End => model::step(s, isize::MAX / 2, m.shift, tx),
        Key::Enter => return ops::begin_rename_selected(s),
        Key::Text(t) if t == " " && !model::typing(s) => ops::quick_look(s),
        _ => return false,
    }
    true
}

/// The selection's look: Finder's accent fill with white text.
pub fn picked_fill(k: &Kit) -> rune::Color {
    k.c.bg_accent
}

// --- list view ----------------------------------------------------------------

pub const ROW_H: f32 = 24.0;
const COLUMNS: [(&str, &str, f32, Align); 4] =
    [("name", "이름", 0.0, Align::Start), ("modified", "수정일", 190.0, Align::Start), ("size", "크기", 90.0, Align::End), ("kind", "종류", 150.0, Align::Start)];

fn sized(e: El, w: f32) -> El {
    if w == 0.0 { e.w(0.0).grow(1.0).min_w(160.0) } else { e.w(w).shrink() }
}

fn header(s: &App, k: &Kit) -> El {
    let mut h = row().key("header").w_full().h(28.0).px(14.0).border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak);
    for (i, (id, title, w, align)) in COLUMNS.into_iter().enumerate() {
        let sorted = (s.sort.0 == id).then_some(s.sort.1);
        let mut inner = row().gap(2.0).center_y().child(text(title).font(k.ty.text_small_medium).color(if sorted.is_some() { k.c.text_base } else { k.c.text_muted }));
        if let Some(d) = sorted {
            inner = inner.child(icon(if d == SortDir::Ascending { k.glyph.chevron_up } else { k.glyph.chevron_down }, 14.0).color(k.c.text_base));
        }
        let mut cell = row().key(id).role("columnheader").label(title).h_full().px(6.0).center_y().cursor(rune::Cursor::Pointer).hover(k.c.hover);
        cell = if align == Align::End { cell.justify_end() } else { cell };
        if i > 0 {
            cell = cell.border_sides([0.0, 0.0, 0.0, 1.0], k.c.border_weak);
        }
        h = h.child(sized(cell, w).child(inner).on_press(move |s: &mut App| {
            let dir = if s.sort.0 == id { s.sort.1.flip() } else if id == "modified" || id == "size" { SortDir::Descending } else { SortDir::Ascending };
            s.sort = (id.to_owned(), dir);
            model::refresh(s);
        }));
    }
    h
}

fn list_row(s: &App, k: &Kit, i: usize) -> El {
    let e = &s.all[s.rows[i]];
    let selected = model::is_selected(s, &e.id);
    let over = s.drop_over.as_deref() == Some(e.id.as_str());
    let (ink, muted) = if selected { (rune::Color::WHITE, rune::Color::WHITE.alpha(0.85)) } else { (k.c.text_base, k.c.text_muted) };
    let cell = |t: String, w: f32, align: Align| sized(row().h_full().px(6.0).center_y(), w).child(text(t.clone()).font(k.ty.text_small).color(muted).align(align).truncate().shrink().min_w(0.0).w_full().help_when_truncated(t));
    let name = if s.renaming.as_deref() == Some(e.id.as_str()) {
        rename_field(s, k, 20.0).grow(1.0)
    } else {
        text(e.name.clone()).font(k.ty.text_small).color(ink).truncate_middle().shrink().min_w(0.0)
    };
    let path = s.dir.join(&e.id);
    let mut r = row()
        .key(e.id.clone())
        .role("row")
        .focus_selected(selected)
        .w_full()
        .h(ROW_H)
        .px(8.0)
        .center_y()
        .radius(5.0)
        .child(sized(row().h_full().px(6.0).gap(6.0).center_y(), 0.0).child(item_icon(s, e, &path, 16.0)).child(name))
        .child(cell(format::date(e.modified), COLUMNS[1].2, Align::Start))
        .child(cell(if e.dir { "--".into() } else { format::size(e.size) }, COLUMNS[2].2, Align::End))
        .child(cell(e.kind.clone(), COLUMNS[3].2, Align::Start));
    r = if selected || over {
        r.fill(picked_fill(k))
    } else if i % 2 == 1 {
        r.fill(k.alpha(k.c.text_base, if k.dark { 0.04 } else { 0.035 }))
    } else {
        r
    };
    item(r, s, e, &e.id, Press::Pick)
}

/// Finder's list: a sortable header and striped rows that select, drag and open.
pub fn list(s: &App, k: &Kit) -> El {
    let kit = k.clone();
    let rows = col()
        .key("rows")
        .w_full()
        .px(6.0)
        .py(4.0)
        .growth(rune::GrowthSpec::default())
        .virtual_rows(s.rows.len(), ROW_H, |s: &App, i| Cow::Borrowed(s.all[s.rows[i]].id.as_str()), move |s: &App, i| list_row(s, &kit, i));
    let body = col().key("list").w_full().grow(1.0).min_h(0.0).scroll(rune::ScrollSpec { stick_bottom: false }).child(rows).on_press(|s: &mut App| {
        // A click below the rows clears the selection, as in Finder.
        if s.renaming.is_some() {
            ops::commit_rename(s);
        }
        model::clear_selection(s);
    });
    col()
        .key("listview")
        .role("grid")
        .label("파일")
        .focusable(true)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .on_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| view_keys(s, ev, tx, 1, false))
        .child(header(s, k))
        .child(body)
}

// --- icon view ----------------------------------------------------------------

pub const CELL_W: f32 = 104.0;
pub const CELL_H: f32 = 112.0;
const ICON: f32 = 64.0;

/// How many icons fit across `width` points.
pub fn columns(width: f32) -> usize {
    ((width - 16.0) / CELL_W).floor().max(1.0) as usize
}

/// An item's name under its icon: Finder's blue pill when selected.
pub fn icon_label(s: &App, k: &Kit, e: &Entry, width: f32) -> El {
    if s.renaming.as_deref() == Some(e.id.as_str()) {
        return rename_field(s, k, 22.0).w(width);
    }
    let selected = model::is_selected(s, &e.id);
    let name = text(e.name.clone()).font(k.ty.text_small).align(Align::Center).wrap_anywhere().max_lines(2).max_w(width - 4.0);
    let pill = row().key("label").px(4.0).py(1.0).radius(4.0).max_w(width);
    if selected { pill.fill(picked_fill(k)).child(name.color(rune::Color::WHITE)) } else { pill.child(name.color(k.c.text_base)) }
}

fn icon_cell(s: &App, k: &Kit, e: &Entry) -> El {
    let selected = model::is_selected(s, &e.id);
    let over = s.drop_over.as_deref() == Some(e.id.as_str());
    let glyph = row()
        .key("glyph")
        .w(ICON + 10.0)
        .h(ICON + 10.0)
        .center()
        .radius(6.0)
        .fill(if selected || over { k.alpha(k.c.text_base, if k.dark { 0.14 } else { 0.1 }) } else { rune::Color::TRANSPARENT })
        .child(item_icon(s, e, &s.dir.join(&e.id), ICON));
    let cell = col()
        .key(e.id.clone())
        .role("gridcell")
        .label(e.name.clone())
        .w(CELL_W)
        .h(CELL_H)
        .pt(6.0)
        .gap(4.0)
        .items_center()
        .child(glyph)
        .child(icon_label(s, k, e, CELL_W - 6.0));
    item(cell, s, e, &e.id, Press::Pick)
}

pub fn icons(s: &App, k: &Kit, cols: usize) -> El {
    let cols = cols.max(1);
    let lines = s.rows.len().div_ceil(cols);
    let kit = k.clone();
    let rows = col().key("lines").w_full().px(8.0).py(8.0).growth(rune::GrowthSpec::default()).virtual_rows(
        lines,
        CELL_H,
        |_: &App, i| Cow::Owned(format!("line:{i}")),
        move |s: &App, line| {
            let mut r = row().w_full().h(CELL_H);
            for &i in s.rows.iter().skip(line * cols).take(cols) {
                r = r.child(icon_cell(s, &kit, &s.all[i]));
            }
            r
        },
    );
    col()
        .key("icons")
        .role("grid")
        .label("파일")
        .focusable(true)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(rune::ScrollSpec { stick_bottom: false })
        .on_press(|s: &mut App| {
            // A click on empty space clears the selection, as in Finder.
            if s.renaming.is_some() {
                ops::commit_rename(s);
            }
            model::clear_selection(s);
        })
        .on_key(move |s: &mut App, ev: &KeyEvent, tx: &mut Tx| view_keys(s, ev, tx, cols as isize, true))
        .child(rows)
}
