//! Toolbar glyphs (drawn in `currentColor`) and Finder-style file icons (drawn in their own
//! colors, at 16 pt in the list and 64 pt in the icon view).
use rune::{El, Icon, icon};

use crate::format::Look;

pub static BACK: Icon = Icon {
    name: "rf-back",
    view: 16.0,
    body: r#"<path d="M10 3L5 8L10 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static FORWARD: Icon = Icon {
    name: "rf-forward",
    view: 16.0,
    body: r#"<path d="M6 3L11 8L6 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static DISK: Icon = Icon {
    name: "rf-disk",
    view: 16.0,
    body: r#"<rect x="2.5" y="4.5" width="11" height="7" rx="1.5" stroke="currentColor" stroke-width="1.2" fill="none"/><circle cx="11" cy="8" r="0.9" fill="currentColor"/>"#,
};
pub static EJECT: Icon = Icon {
    name: "rf-eject",
    view: 16.0,
    body: r#"<path d="M8 3.5L12.5 9H3.5Z" fill="currentColor"/><rect x="3.5" y="10.5" width="9" height="1.6" rx="0.5" fill="currentColor"/>"#,
};
pub static VIEW_ICONS: Icon = Icon {
    name: "rf-view-icons",
    view: 16.0,
    body: r#"<rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/><rect x="9" y="2.5" width="4.5" height="4.5" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/><rect x="2.5" y="9" width="4.5" height="4.5" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/><rect x="9" y="9" width="4.5" height="4.5" rx="1" stroke="currentColor" stroke-width="1.2" fill="none"/>"#,
};
pub static VIEW_LIST: Icon = Icon {
    name: "rf-view-list",
    view: 16.0,
    body: r#"<path d="M2.5 4H13.5M2.5 8H13.5M2.5 12H13.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#,
};
pub static APPS: Icon = Icon {
    name: "rf-apps",
    view: 16.0,
    body: r#"<path d="M8 2.5L13.5 13.5H2.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/><path d="M5.3 9.5H10.7" stroke="currentColor" stroke-width="1.2"/>"#,
};
pub static HOUSE: Icon = Icon {
    name: "rf-house",
    view: 16.0,
    body: r#"<path d="M2.5 7.5L8 3L13.5 7.5M4 6.5V13H12V6.5" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/>"#,
};
pub static DESKTOP: Icon = Icon {
    name: "rf-desktop",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="8" rx="1.2" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M6 13.5H10M8 11V13.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#,
};
pub static DOCS: Icon = Icon {
    name: "rf-docs",
    view: 16.0,
    body: r#"<path d="M4 2.5H9.5L12 5V13.5H4Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/><path d="M6 8H10M6 10.5H10" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"#,
};
pub static DOWNLOADS: Icon = Icon {
    name: "rf-downloads",
    view: 16.0,
    body: r#"<circle cx="8" cy="8" r="5.6" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M8 4.8V10.6M5.6 8.4L8 10.8L10.4 8.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static CLOUD: Icon = Icon {
    name: "rf-cloud",
    view: 16.0,
    body: r#"<path d="M4.8 12.5H11.6A2.6 2.6 0 0 0 11.9 7.3A3.6 3.6 0 0 0 5 6.6A2.95 2.95 0 0 0 4.8 12.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/>"#,
};
pub static TRASH: Icon = Icon {
    name: "rf-trash",
    view: 16.0,
    body: r#"<path d="M3 4.5H13M6.5 4.5V3H9.5V4.5M4.3 4.5L5 13.5H11L11.7 4.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};

static FOLDER: Icon = Icon {
    name: "rf-file-folder",
    view: 64.0,
    body: r##"<path d="M5 15a4 4 0 0 1 4-4h15.5l5 5H55a4 4 0 0 1 4 4v31a4 4 0 0 1-4 4H9a4 4 0 0 1-4-4z" fill="#2F8FE0"/><rect x="5" y="20" width="54" height="35" rx="4" fill="#5DB3F6"/><rect x="5" y="20" width="54" height="1.4" fill="#8CCBFA"/>"##,
};
static APP: Icon = Icon {
    name: "rf-file-app",
    view: 64.0,
    body: r##"<rect x="8" y="8" width="48" height="48" rx="11" fill="#7D8590"/><rect x="8" y="8" width="48" height="24" rx="11" fill="#9AA2AD"/><path d="M32 18L44 44H39.5L32 27L24.5 44H20Z" fill="#FFFFFF"/><path d="M24 37H40" stroke="#FFFFFF" stroke-width="3.5"/>"##,
};
/// A sheet of paper with a folded corner, under every document icon.
macro_rules! page {
    () => {
        r##"<path d="M15 5h25l12 12v40a2 2 0 0 1-2 2H15a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" fill="#FFFFFF" stroke="#BFC1C6" stroke-width="1.2"/><path d="M40 5v10a2 2 0 0 0 2 2h10" fill="#E6E7EA" stroke="#BFC1C6" stroke-width="1.2" stroke-linejoin="round"/>"##
    };
}
static DOCUMENT: Icon = Icon { name: "rf-file-doc", view: 64.0, body: page!() };
static TEXT: Icon = Icon {
    name: "rf-file-text",
    view: 64.0,
    body: concat!(
        page!(),
        r##"<path d="M19 24H46M19 29H46M19 34H46M19 39H46M19 44H38" stroke="#B8BBC2" stroke-width="1.6" stroke-linecap="round"/>"##
    ),
};
static IMAGE: Icon = Icon {
    name: "rf-file-image",
    view: 64.0,
    body: concat!(
        page!(),
        r##"<rect x="18" y="24" width="29" height="22" rx="2" fill="#7CC4F8"/><path d="M18 42L26 33L32 39L37 34L47 44V44a2 2 0 0 1-2 2H20a2 2 0 0 1-2-2z" fill="#4CAF50"/><circle cx="40" cy="30" r="3" fill="#FFE066"/>"##
    ),
};
static ARCHIVE: Icon = Icon {
    name: "rf-file-archive",
    view: 64.0,
    body: concat!(
        page!(),
        r##"<path d="M31 5V13M31 17V21M31 25V29" stroke="#8A8D94" stroke-width="3"/><rect x="28" y="31" width="6" height="9" rx="1.5" fill="#8A8D94"/>"##
    ),
};
static MEDIA: Icon = Icon {
    name: "rf-file-media",
    view: 64.0,
    body: concat!(
        page!(),
        r##"<circle cx="32" cy="35" r="11" fill="#F05A7E"/><path d="M29 29.5V40.5L38.5 35Z" fill="#FFFFFF"/>"##
    ),
};

/// The file icon for `look` at `size` points.
pub fn file(look: Look, size: f32) -> El {
    let i = match look {
        Look::Folder => &FOLDER,
        Look::App => &APP,
        Look::Image => &IMAGE,
        Look::Text => &TEXT,
        Look::Archive => &ARCHIVE,
        Look::Media => &MEDIA,
        Look::Document => &DOCUMENT,
    };
    icon(i, size)
}
