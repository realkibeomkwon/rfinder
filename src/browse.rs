//! The column view (⌘3) and the gallery view (⌘4).
use std::borrow::Cow;
use std::path::Path;

use rune::{Align, El, KeyEvent, SurfaceFit, Tx, col, el, icon, row, text};
use rune_kit::{Kit, glyphs};

use crate::files::{self, Press};
use crate::format::{self, Look};
use crate::model::{self, App, Entry};

// --- column view --------------------------------------------------------------

const COL_W: f32 = 230.0;
const PREVIEW_W: f32 = 280.0;

fn col_row(s: &App, k: &Kit, i: usize, e: &Entry, active: bool) -> El {
    let c = &s.cols[i];
    let id = model::col_id(s, &c.dir, &e.name);
    let picked = c.pick.as_deref() == Some(e.name.as_str());
    let over = s.drop_over.as_deref() == Some(id.as_str());
    // The picked row in the column the keyboard is in is blue; picks to its left stay gray.
    let (fill, ink) = match (picked || over, active) {
        (true, true) => (files::picked_fill(k), rune::Color::WHITE),
        (true, false) => (k.alpha(k.c.text_base, if k.dark { 0.16 } else { 0.12 }), k.c.text_base),
        _ => (rune::Color::TRANSPARENT, k.c.text_base),
    };
    let name = if s.renaming.as_deref() == Some(id.as_str()) {
        files::rename_field(s, k, 20.0).grow(1.0)
    } else {
        text(e.name.clone()).font(k.ty.text_small).color(ink).truncate_middle().shrink().min_w(0.0).grow(1.0)
    };
    let mut r = row()
        .key(id.clone())
        .role("row")
        .focus_selected(picked)
        .w_full()
        .h(files::ROW_H)
        .px(6.0)
        .gap(6.0)
        .center_y()
        .radius(5.0)
        .fill(fill)
        .child(files::item_icon(s, e, &c.dir.join(&e.name), 16.0))
        .child(name);
    if e.is_folder() {
        r = r.child(icon(&glyphs::CARET_RIGHT, 10.0).color(if picked && active { rune::Color::WHITE } else { k.c.icon_muted }));
    }
    files::item(r, s, e, &id, Press::Column(i))
}

fn column(s: &App, k: &Kit, i: usize) -> El {
    let active = model::active_col(s) == i;
    let kit = k.clone();
    let rows = col().key("rows").w_full().px(4.0).py(4.0).growth(rune::GrowthSpec::default()).virtual_rows(
        s.cols[i].entries.len(),
        files::ROW_H,
        move |s: &App, j| Cow::Owned(s.cols[i].entries[j].name.clone()),
        move |s: &App, j| col_row(s, &kit, i, &s.cols[i].entries[j], active),
    );
    let dir = s.cols[i].dir.clone();
    col()
        .key(format!("col:{i}"))
        .w(COL_W)
        .h_full()
        .shrink()
        .border_sides([0.0, 1.0, 0.0, 0.0], k.c.border_weak)
        .scroll(rune::ScrollSpec { stick_bottom: false })
        // A click on a column's empty space keeps its folder picked and closes the columns after.
        .on_press(move |s: &mut App| {
            if i > 0 {
                if let Some(name) = s.cols[i - 1].pick.clone() {
                    model::col_pick(s, i - 1, &name);
                }
            } else {
                s.cols.truncate(1);
                s.cols[0].pick = None;
                model::clear_selection(s);
            }
        })
        .on_drop_files(move |s: &mut App, files, tx| crate::ops::drop_into(s, &dir, files, tx.mods().alt))
        .child(rows)
}

fn preview_image(s: &App, e: &Entry, path: &Path, size: f32) -> El {
    // Images preview their pixels; everything else shows its icon large.
    if let (Some(cache), Look::Image) = (&s.images, e.look) {
        let img = cache.file(path, size, 2.0);
        if img.is_ready() && img.error().is_none() {
            return el().w(size).h(size).pixels_fit(img.pixels(), SurfaceFit::Fit);
        }
    }
    files::item_icon(s, e, path, size.min(256.0))
}

fn fact(k: &Kit, label: &str, value: String) -> El {
    row()
        .w_full()
        .gap(8.0)
        .child(text(label.to_owned()).font(k.ty.text_small).color(k.c.text_muted).w(64.0).align(Align::End))
        .child(text(value).font(k.ty.text_small).color(k.c.text_base).truncate().shrink().min_w(0.0))
}

/// The preview beside the last column when a file is picked, as Finder's column view shows.
fn preview(s: &App, k: &Kit, path: &Path, e: &Entry, width: f32) -> El {
    let size = if e.dir { "--".to_owned() } else { format::size(e.size) };
    col()
        .key("preview")
        .w(width)
        .h_full()
        .shrink()
        .padding(16.0)
        .gap(10.0)
        .items_center()
        .child(row().w_full().h(width.min(220.0)).center().child(preview_image(s, e, path, (width - 40.0).clamp(64.0, 200.0))))
        .child(text(e.name.clone()).font(k.ty.ui_medium).color(k.c.text_base).align(Align::Center).wrap_anywhere().max_lines(3).w_full())
        .child(text(format!("{} - {size}", e.kind)).font(k.ty.text_small).color(k.c.text_muted).align(Align::Center))
        .child(row().w_full().h(1.0).fill(k.c.border_weak).my(4.0))
        .child(text("정보").font(k.ty.text_small_medium).color(k.c.text_base).w_full())
        .child(fact(k, "생성일", format::date(e.created)))
        .child(fact(k, "수정일", format::date(e.modified)))
}

pub fn columns(s: &App, k: &Kit) -> El {
    let mut strip = row().key("columns").h_full().min_w(0.0);
    for i in 0..s.cols.len() {
        strip = strip.child(column(s, k, i));
    }
    let picked_file = model::focus_id(s).and_then(|id| model::entry(s, &id).filter(|e| !e.is_folder()).map(|e| (s.dir.join(&id), e)));
    if let Some((path, e)) = picked_file {
        strip = strip.child(preview(s, k, &path, e, PREVIEW_W));
    }
    col()
        .key("columnview")
        .role("tree")
        .label("파일")
        .focusable(true)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll_x()
        .on_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| files::view_keys(s, ev, tx, 1, false))
        .child(strip)
}

// --- gallery view -------------------------------------------------------------

const THUMB: f32 = 56.0;

fn thumb(s: &App, k: &Kit, e: &Entry) -> El {
    let selected = model::is_selected(s, &e.id);
    let cell = col()
        .key(e.id.clone())
        .role("option")
        .label(e.name.clone())
        .w(THUMB + 16.0)
        .h(THUMB + 16.0)
        .center()
        .radius(8.0)
        .fill(if selected { k.alpha(k.c.text_base, if k.dark { 0.18 } else { 0.12 }) } else { rune::Color::TRANSPARENT })
        .border(if selected { 2.0 } else { 0.0 }, if selected { files::picked_fill(k) } else { rune::Color::TRANSPARENT })
        .child(files::item_icon(s, e, &s.dir.join(&e.id), THUMB));
    files::item(cell, s, e, &e.id, Press::Pick)
}

/// Finder's gallery: the picked item large, its name and facts, and a strip of every item.
pub fn gallery(s: &App, k: &Kit) -> El {
    let picked = model::focus_id(s).and_then(|id| model::entry(s, &id).map(|e| (s.dir.join(&id), e)));
    let stage = match picked {
        Some((path, e)) => col()
            .key("stage")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .center()
            .gap(10.0)
            .padding(16.0)
            .child(preview_image(s, e, &path, 320.0))
            .child(text(e.name.clone()).font(k.ty.ui_medium).color(k.c.text_base).align(Align::Center))
            .child(text(format!("{} - {} - 수정일 {}", e.kind, if e.dir { "--".to_owned() } else { format::size(e.size) }, format::date(e.modified))).font(k.ty.text_small).color(k.c.text_muted)),
        None => col().key("stage").w_full().grow(1.0).center().child(text("선택된 항목 없음").font(k.ty.ui).color(k.c.text_faint)),
    };
    let mut strip = row().key("thumbs").h(THUMB + 24.0).px(10.0).gap(4.0).center_y();
    for &i in &s.rows {
        strip = strip.child(thumb(s, k, &s.all[i]));
    }
    col()
        .key("galleryview")
        .role("listbox")
        .label("파일")
        .focusable(true)
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .on_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| files::view_keys(s, ev, tx, 1, true))
        .child(stage)
        .child(col().key("strip").w_full().h(THUMB + 26.0).border_sides([1.0, 0.0, 0.0, 0.0], k.c.border_weak).scroll_x().child(strip))
}
