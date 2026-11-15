//! rfinder: a small, fast Finder-style file browser built on Rune.
//! `cargo run --release` opens the window; `-- --snap out.png` renders it headless.
mod browse;
mod chrome;
mod commands;
mod dialogs;
mod files;
mod format;
mod icons;
mod model;
mod ops;

use rune::{AppSpec, Chrome, Cx, El, KeyEvent, Tx, Waker, col, el};
use rune_kit::shell::{self, SplitView};
use rune_kit::{Kit, MenuItem, context_menu};

use model::{App, View};

fn context_menu_el(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let some = model::selected_len(s) > 0;
    let one = model::selected_len(s) == 1;
    let item = |label: String, keys: &str, on: bool, f: fn(&mut App, &mut Tx)| {
        MenuItem::new(label).shortcut(keys.to_owned()).disabled(!on).on_select_tx(move |s: &mut App, tx: &mut Tx| {
            s.menu = None;
            f(s, tx);
        })
    };
    let check = |label: &str, on: bool, f: fn(&mut App, &mut Tx)| {
        MenuItem::new(label.to_owned()).checked(on).on_select_tx(move |s: &mut App, tx: &mut Tx| {
            s.menu = None;
            f(s, tx);
        })
    };
    // The menu opens at the pointer, so its trigger is an empty anchor outside the list's layout.
    // As wide as its ⌥ alternates need, so the menu keeps its width while ⌥ goes down and up.
    let menu = context_menu("file-menu", el(), s.menu).min_width(260.0);
    // Holding ⌥ swaps items for their alternates, as Finder's menu does.
    let alt = s.alt;
    let menu = if some {
        let ids = model::selected_ids(s);
        let what = match ids.as_slice() {
            [id] => format!("“{}”", model::entry(s, id).map_or(id.as_str(), |e| e.name.as_str())),
            _ => format!("{}개 항목", ids.len()),
        };
        let mut m = menu.item(item("열기".into(), "⌘O", true, |s, _| ops::open_selected(s))).separator();
        m = if alt {
            m.item(item("즉시 삭제…".into(), "⌥⌘⌫", true, |s, _| ops::ask_delete(s)))
        } else {
            m.item(item("휴지통으로 이동".into(), "⌘⌫", true, |s, _| ops::trash(s)))
        };
        m = m.separator();
        m = if alt {
            m.item(item("속성 보기".into(), "⌥⌘I", true, |s, _| ops::get_info(s)))
        } else {
            m.item(item("정보 가져오기".into(), "⌘I", true, |s, _| ops::get_info(s)))
        };
        m = m
            .item(item("이름 변경".into(), "↩", one, |s, _| {
                ops::begin_rename_selected(s);
            }))
            .item(item(format!("{what} 압축"), "", true, |s, _| ops::compress(s)))
            .item(item("복제".into(), "⌘D", true, |s, _| ops::duplicate(s)))
            .item(item(if alt { format!("{what} 슬라이드쇼") } else { format!("{what} 훑어보기") }, if alt { "⌥Space" } else { "Space" }, true, |s, _| ops::quick_look(s)))
            .separator();
        m = if alt {
            m.item(item(format!("{what} 경로 이름으로 복사"), "⌥⌘C", true, ops::copy_path))
        } else {
            m.item(item(format!("{what} 복사"), "⌘C", true, ops::copy))
        };
        if model::searching(s) {
            m = m.item(item("둘러싸는 폴더에서 보기".into(), "⌘R", one, ops::show_in_folder));
        }
        m
    } else {
        let info = if alt { item("속성 보기".into(), "⌥⌘I", true, |s, _| ops::get_info(s)) } else { item("정보 가져오기".into(), "⌘I", true, |s, _| ops::get_info(s)) };
        menu.item(item("새로운 폴더".into(), "⇧⌘N", !model::searching(s), |s, _| ops::new_folder(s)))
            .separator()
            .item(info)
            .separator()
            .item(item("항목 붙여넣기".into(), "⌘V", !model::searching(s), |s, tx| ops::paste(s, false, tx)))
            .separator()
            .item(check("아이콘으로 보기", s.view == View::Icons, |s, _| model::set_view(s, View::Icons)))
            .item(check("목록으로 보기", s.view == View::List, |s, _| model::set_view(s, View::List)))
            .item(check("계층으로 보기", s.view == View::Columns, |s, _| model::set_view(s, View::Columns)))
            .item(check("갤러리로 보기", s.view == View::Gallery, |s, _| model::set_view(s, View::Gallery)))
            .item(item(if s.hidden { "숨김 파일 가리기" } else { "숨김 파일 보기" }.into(), "⇧⌘.", true, |s, _| ops::toggle_hidden(s)))
    };
    menu.on_close(|s: &mut App| s.menu = None).build(k, layer)
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = Kit::neutral(cx.system_dark());
    let mut layer = Vec::new();

    let body = match s.view {
        // A search lists its results whatever the view; Finder's columns can't show them.
        View::List | View::Columns if model::searching(s) => files::list(s, &k),
        View::List => files::list(s, &k),
        View::Columns => browse::columns(s, &k),
        View::Gallery => browse::gallery(s, &k),
        View::Icons => {
            let cols = cx.layout_with("files", |r| files::columns(r[2])).unwrap_or(6);
            files::icons(s, &k, cols)
        }
    };
    let here = s.dir.clone();
    let files = col()
        .key("files")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(body)
        // A right-click on empty space acts on the folder itself.
        .on_secondary_press(|s: &mut App, p, tx| {
            model::clear_selection(s);
            s.alt = tx.mods().alt;
            s.menu = Some(p);
        })
        // Files dropped on empty space land in this folder.
        .on_drop_files(move |s: &mut App, dropped, tx| ops::drop_into(s, &here, dropped, tx.mods().alt));
    let menu = context_menu_el(s, &k, &mut layer);
    dialogs::info(s, &k, &mut layer);
    dialogs::goto(s, &k, &mut layer);
    dialogs::confirm_delete(s, &k, &mut layer);

    let mut main = col().key("main").w_full().h_full().min_h(0.0);
    if model::searching(s) {
        main = main.child(chrome::scope_bar(s, &k));
    }
    main = main.child(files).child(menu).child(chrome::path_bar(s, &k)).child(chrome::status(s, &k));

    let shell = SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, chrome::sidebar(s, &k))
        .toolbar(chrome::toolbar(s, &k))
        .titlebar_inset(cx.titlebar_inset())
        .reduced_motion(cx.is_reduced_motion())
        .build(&k);
    rune_kit::root(shell, layer)
}

fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        return true;
    }
    commands::key(s, ev, tx)
}

/// Set when a file icon or preview finishes decoding, so the next frame swaps it in.
static IMAGES_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn setup(s: &mut App, waker: Waker) {
    s.found.set_waker(waker.clone());
    s.totals.set_waker(waker.clone());
    let images: Waker = std::sync::Arc::new(move || {
        IMAGES_READY.store(true, std::sync::atomic::Ordering::Relaxed);
        waker();
    });
    s.images = Some(rune::ImageCache::new(images));
}

fn poll(s: &mut App, _: f64, _: &mut Tx) -> bool {
    let mut changed = IMAGES_READY.swap(false, std::sync::atomic::Ordering::Relaxed);
    // Space closes Quick Look only while its window is open.
    if s.quick_look.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_some()) {
        s.quick_look = None;
    }
    if let Some(found) = s.found.take() {
        if model::searching(s) {
            s.all = found;
            model::refresh(s);
        }
        changed = true;
    }
    if let Some((path, bytes, items)) = s.totals.take()
        && let Some(info) = s.info.as_mut().filter(|i| i.path == path)
    {
        info.total = Some((bytes, items));
        changed = true;
    }
    changed
}

fn select_first(s: &mut App) {
    let first = model::row_ids(s).next().map(str::to_owned);
    if let Some(id) = first {
        model::select_only(s, id);
    }
}

/// Named states for `--snap out.png --scene <name>`.
type Scene = (&'static str, fn(&mut App));

static SCENES: &[Scene] = &[
    ("icons", |s| {
        s.view = View::Icons;
        select_first(s);
    }),
    ("info", |s| {
        select_first(s);
        ops::get_info(s);
    }),
    ("menu", |s| {
        select_first(s);
        s.menu = Some(rune::PointerPress { x: 520.0, y: 120.0 });
    }),
    ("goto", dialogs::open_goto),
];

fn main() {
    let chrome = Chrome { full_size_content: true, traffic_lights: Some((16.0, 17.0)), drag_regions: true };
    let spec = AppSpec {
        on_key: Some(on_key),
        setup: Some(setup),
        poll: Some(poll),
        scenes: SCENES,
        min_size: Some((560.0, 320.0)),
        ..AppSpec::new("rfinder", (920.0, 560.0), model::init, view)
    };
    // Finder's menus swap items the moment ⌥ goes down or up, even while one is open.
    let mut app = rune::App::new(spec).chrome(chrome).on_modifiers(|s: &mut App, mods, _| s.alt = mods.alt);
    for item in commands::menu_bar() {
        app = app.menu_item(item);
    }
    app.run();
}

#[cfg(test)]
mod tests;
