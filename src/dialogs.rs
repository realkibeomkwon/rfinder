//! Get Info (⌘I) and Go to Folder (⇧⌘G).
use rune::{Align, El, Key, KeyEvent, Tx, col, row, text};
use rune_kit::{Button, ButtonVariant, Dialog, Kit, TextInput};

use crate::model::App;
use crate::{format, icons, ops};

fn line(k: &Kit, label: &str, value: String) -> El {
    row()
        .w_full()
        .gap(8.0)
        .items_start()
        .child(text(format!("{label}:")).font(k.ty.text_small_medium).color(k.c.text_muted).align(Align::End).w(84.0))
        .child(text(value).font(k.ty.text_small).color(k.c.text_base).wrap_anywhere().grow(1.0).min_w(0.0))
}

pub fn info(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(info) = &s.info else { return };
    let e = &info.entry;
    let size = match (e.dir, info.total) {
        (false, _) => format::size_detail(e.size),
        (true, Some((bytes, items))) => format!("{}, {items}개 항목", format::size_detail(bytes)),
        (true, None) => "계산 중…".into(),
    };
    let where_ = info.path.parent().map_or_else(|| "/".into(), |p| p.display().to_string());
    let header = row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(icons::file(e.look, 48.0))
        .child(col().gap(2.0).grow(1.0).min_w(0.0).child(text(e.name.clone()).font(k.ty.ui_medium).color(k.c.text_base).truncate_middle()).child(text(size.clone()).font(k.ty.text_small).color(k.c.text_muted)))
        .child(text(format::date(e.modified)).font(k.ty.text_small).color(k.c.text_muted));
    let body = col()
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(6.0)
        .child(header)
        .child(row().w_full().h(1.0).fill(k.c.border_weak).my(6.0))
        .child(line(k, "종류", e.kind.clone()))
        .child(line(k, "크기", size))
        .child(line(k, "위치", where_))
        .child(line(k, "생성일", format::date(e.created)))
        .child(line(k, "수정일", format::date(e.modified)));
    Dialog::new("info", format!("{} 정보", e.name)).open(true).fit().width(340.0).child(body).on_close(|s: &mut App| s.info = None).build(k, layer);
}

pub fn goto(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(edit) = &s.goto else { return };
    let field = TextInput::new(edit)
        .label("폴더 경로")
        .placeholder("/Users/…")
        .on_edit(|s: &mut App| s.goto.as_mut().expect("goto is open while its field is shown"))
        .on_submit(|s: &mut App, tx: &mut Tx| {
            let typed = s.goto.as_ref().map(|e| e.text.clone()).unwrap_or_default();
            ops::goto(s, &typed, tx);
        })
        .on_key(|s: &mut App, ev: &KeyEvent, _| {
            let esc = ev.key == Key::Escape;
            if esc {
                s.goto = None;
            }
            esc
        })
        .build(k)
        .w_full();
    let go = Button::new("이동")
        .variant(ButtonVariant::Submit)
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            let typed = s.goto.as_ref().map(|e| e.text.clone()).unwrap_or_default();
            ops::goto(s, &typed, tx);
        })
        .build(k);
    let body = col().w_full().px(16.0).pb(16.0).gap(12.0).child(field).child(row().w_full().justify_end().child(go));
    Dialog::new("goto", "폴더로 이동").open(true).fit().width(420.0).child(body).on_close(|s: &mut App| s.goto = None).build(k, layer);
}

pub fn open_goto(s: &mut App) {
    let mut e = rune::TextEdit::new(s.dir.to_string_lossy().into_owned());
    e.anchor = 0;
    e.focus = e.text.len();
    e.focused = true;
    s.goto = Some(e);
}

/// ⌥⌘⌫'s question, worded as Finder's.
pub fn confirm_delete(s: &App, k: &Kit, layer: &mut Vec<El>) {
    let Some(paths) = &s.confirm_delete else { return };
    let what = match paths.as_slice() {
        [one] => format!("“{}” 항목을", crate::model::display_name(one)),
        many => format!("{}개 항목을", many.len()),
    };
    let cancel = Button::new("취소").on_press(|s: &mut App| s.confirm_delete = None).build(k);
    let delete = Button::new("삭제").variant(ButtonVariant::Danger).on_press(ops::delete_now).build(k);
    let body = col()
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(14.0)
        .child(text("이 동작은 실행 취소할 수 없습니다.").font(k.ty.text_small).color(k.c.text_muted))
        .child(row().w_full().gap(8.0).justify_end().child(cancel).child(delete));
    Dialog::new("delete", format!("{what} 즉시 삭제하겠습니까?")).open(true).fit().width(380.0).click_outside(false).child(body).on_close(|s: &mut App| s.confirm_delete = None).build(k, layer);
}
