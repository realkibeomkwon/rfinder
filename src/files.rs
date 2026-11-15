//! The folder's items: the list view (a sortable table) and the icon view (a grid).
use std::borrow::Cow;
use std::path::PathBuf;

use rune::{Align, DragItem, El, Key, KeyEvent, Tx, col, row, text};
use rune_kit::{Cell, Kit, Table, TableColumn, TextInput};

use crate::model::{self, App, Entry};
use crate::{format, icons, ops};

/// What a drag of `id` carries: the whole selection when `id` is in it, else `id` alone.
fn drag_item(s: &App, id: &str) -> Option<DragItem> {
    let paths: Vec<PathBuf> = if model::is_selected(s, id) { model::selected(s) } else { vec![s.dir.join(id)] };
    let mut item = DragItem::new().allow_move().within_app();
    for p in paths {
        item = item.file(p);
    }
    Some(item)
}

/// Makes `e` the right-click and (for folders) drop target of item `id`.
fn item_target(e: El, s: &App, entry: &Entry) -> El {
    let id = entry.id.clone();
    let menu_id = id.clone();
    let mut e = e
        .on_secondary_press(move |s: &mut App, p, _| {
            // Finder acts on what was right-clicked: the selection if it's in it, else it alone.
            if !model::is_selected(s, &menu_id) {
                model::select_only(s, menu_id.clone());
            }
            s.menu = Some(p);
        });
    if entry.is_folder() {
        let (over, target) = (id.clone(), s.dir.join(&id));
        e = e
            .on_drop_files(move |s: &mut App, files, tx| {
                s.drop_over = None;
                ops::drop_into(s, &target, files, tx.mods().alt);
            })
            .on_drop_hover(move |s: &mut App, on, _| s.drop_over = on.then(|| over.clone()));
    }
    e
}

fn rename_field(s: &App, k: &Kit, multiline: bool) -> El {
    TextInput::new(&s.rename)
        .label("이름 변경")
        .height(if multiline { 22.0 } else { 20.0 })
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

// --- list view --------------------------------------------------------------

pub fn list(s: &App, k: &Kit) -> El {
    let kit = k.clone();
    Table::new(|s: &App| &s.table, |s: &mut App| &mut s.table, |s: &App| s.rows.len(), |s: &App, i| Cow::Borrowed(s.all[s.rows[i]].id.as_str()))
        .name("파일")
        .column(TableColumn::new("name", "이름").flex(3.0).min(180.0).sortable())
        .column(TableColumn::new("modified", "수정일").width(200.0).min(120.0).sortable())
        .column(TableColumn::new("size", "크기").width(90.0).min(60.0).align(Align::End).sortable())
        .column(TableColumn::new("kind", "종류").width(150.0).min(80.0).sortable())
        .cell(move |s: &App, i, column| {
            let k = &kit;
            let e = &s.all[s.rows[i]];
            let muted = |t: String, align: Align| text(t.clone()).font(k.ty.text_small).color(k.c.text_muted).align(align).truncate().shrink().min_w(0.0).help_when_truncated(t);
            let content = match column {
                "name" => {
                    let label = if s.renaming.as_deref() == Some(e.id.as_str()) {
                        rename_field(s, k, false).grow(1.0)
                    } else {
                        text(e.name.clone()).font(k.ty.text_small).color(k.c.text_base).truncate_middle().shrink().min_w(0.0)
                    };
                    let over = s.drop_over.as_deref() == Some(e.id.as_str());
                    let mut r = row().gap(6.0).center_y().child(icons::file(e.look, 16.0)).child(label);
                    if over {
                        r = r.fill(k.alpha(k.c.text_accent, 0.25)).radius(4.0);
                    }
                    r
                }
                "modified" => row().center_y().child(muted(format::date(e.modified), Align::Start)),
                "size" => row().center_y().justify_end().child(muted(if e.dir { "--".into() } else { format::size(e.size) }, Align::End)),
                _ => row().center_y().child(muted(e.kind.clone(), Align::Start)),
            };
            Cell::el(item_target(content.w_full().h_full().min_w(0.0), s, e))
        })
        .on_sort(|s: &mut App, _, _, _| model::refresh(s))
        .on_open(|s: &mut App, id, tx| {
            // Return renames in Finder; a double click (or ⌘↓, ⌘O) opens.
            if tx.by_keys() {
                if s.renaming.is_none() {
                    ops::begin_rename(s, id.to_owned());
                }
            } else {
                ops::open(s, id);
            }
        })
        .zebra(true)
        .build(s, k)
        .grow(1.0)
        .min_h(0.0)
}

// --- icon view --------------------------------------------------------------

pub const CELL_W: f32 = 104.0;
pub const CELL_H: f32 = 112.0;
const ICON: f32 = 64.0;

/// How many icons fit across `width` points.
pub fn columns(width: f32) -> usize {
    ((width - 16.0) / CELL_W).floor().max(1.0) as usize
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
        .child(icons::file(e.look, ICON));
    let label = if s.renaming.as_deref() == Some(e.id.as_str()) {
        rename_field(s, k, true).w(CELL_W - 6.0)
    } else {
        let name = text(e.name.clone()).font(k.ty.text_small).align(Align::Center).wrap_anywhere().max_lines(2).max_w(CELL_W - 10.0);
        let pill = row().key("label").px(4.0).py(1.0).radius(4.0).max_w(CELL_W - 6.0);
        if selected {
            pill.fill(k.c.bg_accent).child(name.color(rune::Color::WHITE))
        } else {
            pill.child(name.color(k.c.text_base))
        }
    };
    let (id, open_id, drag_id) = (e.id.clone(), e.id.clone(), e.id.clone());
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
        .child(label)
        .on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if s.renaming.as_deref() != Some(id.as_str()) {
                if s.renaming.is_some() {
                    ops::commit_rename(s);
                }
                model::pick(s, &id, tx.mods());
            }
        })
        .on_double_press(move |s: &mut App| ops::open(s, &open_id))
        // Dragging an icon drags the selection to Finder, another app or a folder here. (The
        // list view's rows don't: a drag source there would take the table's own clicks.)
        .drag_out(move |s: &App| drag_item(s, &drag_id));
    item_target(cell, s, e)
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
        .on_key(move |s: &mut App, ev: &KeyEvent, tx: &mut Tx| {
            let m = ev.mods;
            if m.cmd {
                return false;
            }
            match ev.key {
                Key::Left => model::step_picks(s, -1, 0, cols, m.shift, tx),
                Key::Right => model::step_picks(s, 1, 0, cols, m.shift, tx),
                Key::Up => model::step_picks(s, 0, -1, cols, m.shift, tx),
                Key::Down => model::step_picks(s, 0, 1, cols, m.shift, tx),
                Key::Enter => return ops::begin_rename_selected(s),
                _ => return false,
            }
            true
        })
        .child(rows)
}
