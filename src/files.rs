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

/// The system's icon for `path` (a folder's own, a disk's), as the title and path bar show
/// it; a drawn stand-in until it's decoded or off macOS.
pub fn path_icon(s: &App, path: &Path, size: f32) -> El {
    if let Some(img) = s.images.as_ref().map(|c| c.app_icon(path, size))
        && img.is_ready()
        && img.error().is_none()
    {
        return el().w(size).h(size).pixels_fit(img.pixels(), SurfaceFit::Fit);
    }
    let dir = path.is_dir();
    if path.parent().is_none() {
        return icon(&icons::DISK, size).color(rune::Color::WHITE.mix(rune::Color::BLACK, 0.5));
    }
    icons::file(format::look(&model::display_name(path), dir), size)
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
        Key::Right | Key::Left if model::outline(s) => return model::disclose_focused(s, ev.key == Key::Right, m.alt, tx),
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
    // Lined up with the rows' cells; "이름" sits over the names, past the icons.
    let mut h = row().key("header").w_full().h(28.0).px(14.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak);
    for (i, (id, title, w, align)) in COLUMNS.into_iter().enumerate() {
        let sorted = (s.sort.0 == id).then_some(s.sort.1);
        // Finder's headers are plain grey text; the sorted one is darker, with a chevron at its end.
        let (font, ink) = if sorted.is_some() { (k.ty.text_small_medium, k.c.text_base) } else { (k.ty.text_small, k.c.text_muted) };
        let mut inner = row().w_full().gap(2.0).center_y().child(text(title).font(font).color(ink));
        inner = if align == Align::End { inner.justify_end() } else { inner };
        if let Some(d) = sorted {
            let chevron = icon(if d == SortDir::Ascending { k.glyph.chevron_up } else { k.glyph.chevron_down }, 12.0).color(k.c.icon_muted);
            inner = if align == Align::End { inner.child(chevron) } else { inner.child(el().grow(1.0)).child(chevron) };
        }
        let mut cell = row().key(id).role("columnheader").label(title).h_full().pl(if i == 0 { NAME_INSET } else { 6.0 }).pr(6.0).center_y().cursor(rune::Cursor::Pointer).hover(k.c.hover);
        if i > 0 {
            // A short divider between titles, not a full-height rule.
            cell = cell.child(el().w(1.0).h(14.0).fill(k.c.border_weak).absolute(0.0, 7.0));
        }
        h = h.child(sized(cell, w).child(inner).on_press(move |s: &mut App| {
            let dir = if s.sort.0 == id { s.sort.1.flip() } else if id == "modified" || id == "size" { SortDir::Descending } else { SortDir::Ascending };
            s.sort = (id.to_owned(), dir);
            model::refresh(s);
        }));
    }
    h
}

/// Finder's alternating row fill.
fn stripe(k: &Kit) -> rune::Color {
    k.alpha(k.c.text_base, if k.dark { 0.04 } else { 0.035 })
}

/// Where a name starts in its cell: past the ▸ and the icon, so "이름" lines up over it.
const NAME_INSET: f32 = 6.0 + DISCLOSE + 6.0 + 16.0 + 6.0;
/// The ▸'s slot: its 16 pt chevron overhangs it, leaving Finder's 5 pt before the icon.
const DISCLOSE: f32 = 11.0;
/// An outline level's indent.
const INDENT: f32 = 16.0;

/// The ▸ before a folder in the list: opens its items under it (→), or closes them (←).
fn disclosure(s: &App, e: &Entry, ink: rune::Color) -> El {
    let slot = row().key("disclose").w(DISCLOSE).h_full().center_y();
    if !e.is_folder() || !model::outline(s) {
        return slot;
    }
    let open = model::is_expanded(s, &e.id);
    let id = e.id.clone();
    slot.role("button")
        .label(if open { "접기" } else { "펼치기" })
        .expanded(open)
        .on_press(move |s: &mut App| model::toggle_expanded(s, &id))
        .child(icon(if open { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, 16.0).color(ink))
}

/// Tags after a name: overlapping colour dots, as Finder draws them.
pub fn tag_dots(k: &Kit, e: &Entry) -> Option<El> {
    if e.tags.is_empty() {
        return None;
    }
    let mut dots = row().key("tags").label(e.tags.join(", "));
    for (n, t) in e.tags.iter().take(3).enumerate() {
        dots = dots.child(crate::chrome::tag_dot(k, t, 9.0).ml(if n == 0 { 0.0 } else { -3.0 }));
    }
    Some(dots)
}

/// A group's title line in the grouped list.
fn group_line(k: &Kit, g: usize, title: &str) -> El {
    row()
        .key(group_key(g))
        .role("rowheader")
        .w_full()
        .h(ROW_H)
        .px(14.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak)
        .child(text(title.to_owned()).font(k.ty.text_small_medium).color(k.c.text_base))
}

/// A group line's key: NUL never appears in a file name, so it can't match an item's id.
fn group_key(g: usize) -> String {
    format!("\0group{g}")
}

/// The list's lines: its rows, with each group's title line before its first row.
fn line_count(s: &App) -> usize {
    s.rows.len() + s.groups.len()
}

/// Line `n`: Ok(row index), or Err(group index) for a title line.
fn line_at(s: &App, n: usize) -> Result<usize, usize> {
    let mut before = 0;
    for (g, (start, _)) in s.groups.iter().enumerate() {
        match (start + g).cmp(&n) {
            std::cmp::Ordering::Equal => return Err(g),
            std::cmp::Ordering::Less => before += 1,
            std::cmp::Ordering::Greater => break,
        }
    }
    Ok(n - before)
}

fn list_line(s: &App, k: &Kit, n: usize) -> El {
    match line_at(s, n) {
        Ok(i) => list_row(s, k, i),
        Err(g) => group_line(k, g, &s.groups[g].1),
    }
}

fn line_id(s: &App, n: usize) -> Cow<'_, str> {
    match line_at(s, n) {
        Ok(i) => Cow::Borrowed(s.all[s.rows[i]].id.as_str()),
        Err(g) => Cow::Owned(group_key(g)),
    }
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
    let mut lead = sized(row().h_full().pl(6.0 + e.depth() as f32 * INDENT).pr(6.0).gap(6.0).center_y(), 0.0)
        .child(disclosure(s, e, if selected { rune::Color::WHITE } else { k.alpha(k.c.text_base, 0.5) }))
        .child(item_icon(s, e, &path, 16.0))
        .child(name);
    if let Some(dots) = tag_dots(k, e) {
        lead = lead.child(dots);
    }
    let size = match model::size_of(s, e) {
        Some(b) => format::size(b),
        None => "--".into(),
    };
    let mut r = row()
        .key(e.id.clone())
        .role("row")
        .focus_selected(selected)
        .w_full()
        .h(ROW_H)
        .px(8.0)
        .center_y()
        .radius(5.0)
        .child(lead)
        .child(cell(format::date(e.modified), COLUMNS[1].2, Align::Start))
        .child(cell(size, COLUMNS[2].2, Align::End))
        .child(cell(e.kind.clone(), COLUMNS[3].2, Align::Start));
    r = if selected || over {
        r.fill(picked_fill(k))
    } else if i % 2 == 1 {
        r.fill(stripe(k))
    } else {
        r
    };
    item(r, s, e, &e.id, Press::Pick)
}

/// Finder's list: a sortable header and striped rows that select, drag and open. `height` is
/// the view's, so the stripes carry on below the last row as Finder's do.
pub fn list(s: &App, k: &Kit, height: f32) -> El {
    let kit = k.clone();
    let rows = col()
        .key("rows")
        .w_full()
        .px(6.0)
        .py(4.0)
        .growth(rune::GrowthSpec::default())
        .virtual_rows(line_count(s), ROW_H, line_id, move |s: &App, n| list_line(s, &kit, n));
    let n = line_count(s);
    let spare = ((height - 28.0 - 8.0) / ROW_H - n as f32).floor().max(0.0) as usize;
    let mut blank = col().key("stripes").w_full().px(6.0);
    for i in n..n + spare {
        blank = blank.child(row().w_full().h(ROW_H).radius(5.0).fill(if i % 2 == 1 { stripe(k) } else { rune::Color::TRANSPARENT }));
    }
    let body = col().key("list").w_full().grow(1.0).min_h(0.0).scroll(rune::ScrollSpec { stick_bottom: false }).child(rows).child(blank).on_press(|s: &mut App| {
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
    let pill = row().key("label").px(4.0).py(1.0).gap(3.0).center_y().radius(4.0).max_w(width);
    let pill = if let Some(dots) = tag_dots(k, e) { pill.child(dots) } else { pill };
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

/// A line of the icon view: a group's title, or the rows `start..end` of `s.rows` in a line.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum IconLine {
    Title(usize),
    Items(usize, usize),
}

/// The icon view's lines, `cols` icons to a line; grouped, each group starts a new line under
/// its title, as Finder lays it out.
pub fn icon_lines(s: &App, cols: usize) -> Vec<IconLine> {
    let cols = cols.max(1);
    let mut spans: Vec<(Option<usize>, usize, usize)> = vec![];
    if s.groups.is_empty() {
        spans.push((None, 0, s.rows.len()));
    } else {
        for (g, &(start, _)) in s.groups.iter().enumerate() {
            let end = s.groups.get(g + 1).map_or(s.rows.len(), |n| n.0);
            spans.push((Some(g), start, end));
        }
    }
    let mut lines = vec![];
    for (g, start, end) in spans {
        lines.extend(g.map(IconLine::Title));
        lines.extend((start..end).step_by(cols).map(|a| IconLine::Items(a, (a + cols).min(end))));
    }
    lines
}

const TITLE_H: f32 = 30.0;

/// ↑ and ↓ in a grouped icon view: to the icon above or below in the grid, across a group's
/// title, keeping the column where the line is long enough.
fn grid_step(s: &mut App, cols: usize, down: bool, extend: bool, tx: &mut Tx) {
    let lines: Vec<(usize, usize)> = icon_lines(s, cols).into_iter().filter_map(|l| if let IconLine::Items(a, b) = l { Some((a, b)) } else { None }).collect();
    let at = s.picks.cursor.as_ref().and_then(|c| s.rows.iter().position(|&i| s.all[i].id == *c));
    let target = match at {
        None => if down { 0 } else { s.rows.len().saturating_sub(1) },
        Some(r) => {
            let li = lines.iter().position(|&(a, b)| a <= r && r < b).unwrap_or(0);
            let col = r - lines[li].0;
            let next = if down { (li + 1).min(lines.len() - 1) } else { li.saturating_sub(1) };
            let (a, b) = lines[next];
            (a + col).min(b - 1)
        }
    };
    let Some(id) = s.rows.get(target).map(|&i| s.all[i].id.clone()) else { return };
    if extend {
        model::pick(s, &id, rune::Mods { shift: true, ..Default::default() });
    } else {
        model::select_only(s, id.clone());
    }
    tx.scroll_into_view_in("icons", id);
}

pub fn icons(s: &App, k: &Kit, cols: usize) -> El {
    let cols = cols.max(1);
    let count = icon_lines(s, cols).len();
    let kit = k.clone();
    let rows = col().key("lines").w_full().px(8.0).py(8.0).growth(rune::GrowthSpec::default()).virtual_rows(
        count,
        CELL_H,
        move |s: &App, i| match icon_lines(s, cols).get(i) {
            Some(IconLine::Title(g)) => Cow::Owned(group_key(*g)),
            _ => Cow::Owned(format!("line:{i}")),
        },
        move |s: &App, line| match icon_lines(s, cols).get(line).copied() {
            Some(IconLine::Title(g)) => group_line(&kit, g, &s.groups[g].1).h(TITLE_H).px(6.0),
            Some(IconLine::Items(a, b)) => {
                let mut r = row().w_full().h(CELL_H);
                for &i in &s.rows[a..b] {
                    r = r.child(icon_cell(s, &kit, &s.all[i]));
                }
                r
            }
            None => el(),
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
            let vertical = matches!(ev.key, Key::Up | Key::Down) && !(m.cmd || m.ctrl);
            if vertical && !s.groups.is_empty() {
                grid_step(s, cols, ev.key == Key::Down, m.shift, tx);
                return true;
            }
            view_keys(s, ev, tx, cols as isize, true)
        })
        .child(rows)
}
