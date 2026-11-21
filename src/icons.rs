//! Toolbar glyphs (drawn in `currentColor`) and Finder-style file icons (drawn in their own
//! colors, at 16 pt in the list and 64 pt in the icon view).
use rune::{El, Icon, icon};

use crate::format::Look;

pub static BACK: Icon = Icon {
    name: "rf-back",
    view: 16.0,
    body: r#"<path d="M10.5 2.5L5 8L10.5 13.5" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static FORWARD: Icon = Icon {
    name: "rf-forward",
    view: 16.0,
    body: r#"<path d="M5.5 2.5L11 8L5.5 13.5" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static DISK: Icon = Icon {
    name: "rf-disk",
    view: 16.0,
    body: r#"<rect x="1.5" y="4.2" width="13" height="7.8" rx="2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M1.5 8.9H14.5M3.8 10.5H7.2" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none"/><circle cx="12.2" cy="10.5" r="0.75" fill="currentColor"/>"#,
};
pub static EJECT: Icon = Icon {
    name: "rf-eject",
    view: 16.0,
    body: r#"<path d="M8 3.2L12.8 9H3.2Z" fill="currentColor" stroke="currentColor" stroke-width="0.8" stroke-linejoin="round"/><rect x="3.2" y="10.6" width="9.6" height="1.7" rx="0.6" fill="currentColor"/>"#,
};
pub static VIEW_ICONS: Icon = Icon {
    name: "rf-view-icons",
    view: 16.0,
    body: r#"<rect x="2" y="2" width="5" height="5" rx="1.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="9" y="2" width="5" height="5" rx="1.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="2" y="9" width="5" height="5" rx="1.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="9" y="9" width="5" height="5" rx="1.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static VIEW_LIST: Icon = Icon {
    name: "rf-view-list",
    view: 16.0,
    body: r#"<circle cx="2.9" cy="4" r="1" fill="currentColor"/><circle cx="2.9" cy="8" r="1" fill="currentColor"/><circle cx="2.9" cy="12" r="1" fill="currentColor"/><path d="M6 4H14M6 8H14M6 12H14" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static APPS: Icon = Icon {
    name: "rf-apps",
    view: 16.0,
    body: r#"<path d="M9.4 2.4L3.8 12.6M6.6 2.4L12.2 12.6M2.2 10.2H13.8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static VIEW_COLUMNS: Icon = Icon {
    name: "rf-view-columns",
    view: 16.0,
    body: r#"<rect x="1.5" y="2.5" width="13" height="11" rx="2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M5.8 2.5V13.5M10.2 2.5V13.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static VIEW_GALLERY: Icon = Icon {
    name: "rf-view-gallery",
    view: 16.0,
    body: r#"<rect x="1.5" y="1.8" width="13" height="8.6" rx="1.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="1.6" y="12.2" width="2" height="2" rx="0.5" fill="currentColor"/><rect x="5.3" y="12.2" width="2" height="2" rx="0.5" fill="currentColor"/><rect x="9" y="12.2" width="2" height="2" rx="0.5" fill="currentColor"/><rect x="12.6" y="12.2" width="2" height="2" rx="0.5" fill="currentColor"/>"#,
};
pub static HOUSE: Icon = Icon {
    name: "rf-house",
    view: 16.0,
    body: r#"<path d="M1.8 8.2L8 2.6L14.2 8.2M3.6 6.7V13.4H12.4V6.7M6.6 13.4V10H9.4V13.4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static DESKTOP: Icon = Icon {
    name: "rf-desktop",
    view: 16.0,
    body: r#"<rect x="1.5" y="2.8" width="13" height="10.4" rx="2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M5 10.4H11" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static DOCS: Icon = Icon {
    name: "rf-docs",
    view: 16.0,
    body: r#"<path d="M4.5 1.8H9.2L12.5 5.1V13.2A1 1 0 0 1 11.5 14.2H4.5A1 1 0 0 1 3.5 13.2V2.8A1 1 0 0 1 4.5 1.8ZM9 2V5.3H12.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static DOWNLOADS: Icon = Icon {
    name: "rf-downloads",
    view: 16.0,
    body: r#"<circle cx="8" cy="8" r="6.3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M8 4.6V11M5.4 8.5L8 11.1L10.6 8.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static CLOUD: Icon = Icon {
    name: "rf-cloud",
    view: 16.0,
    body: r#"<path d="M4.8 12.5H11.6A2.6 2.6 0 0 0 11.9 7.3A3.6 3.6 0 0 0 5 6.6A2.95 2.95 0 0 0 4.8 12.5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/>"#,
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

pub static SHARE: Icon = Icon {
    name: "rf-share",
    view: 16.0,
    body: r#"<path d="M5.2 6.2H4.2A1.2 1.2 0 0 0 3 7.4V13.3A1.2 1.2 0 0 0 4.2 14.5H11.8A1.2 1.2 0 0 0 13 13.3V7.4A1.2 1.2 0 0 0 11.8 6.2H10.8M8 1.5V10M5.5 4L8 1.5L10.5 4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static TAG: Icon = Icon {
    name: "rf-tag",
    view: 16.0,
    body: r#"<path d="M2 2.8V7.3A1.2 1.2 0 0 0 2.35 8.15L7.85 13.65A1.2 1.2 0 0 0 9.55 13.65L13.65 9.55A1.2 1.2 0 0 0 13.65 7.85L8.15 2.35A1.2 1.2 0 0 0 7.3 2H2.8A0.8 0.8 0 0 0 2 2.8Z" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><circle cx="5.1" cy="5.1" r="1.1" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static MORE: Icon = Icon {
    name: "rf-more",
    view: 16.0,
    body: r#"<circle cx="3.2" cy="8" r="1.35" fill="currentColor"/><circle cx="8" cy="8" r="1.35" fill="currentColor"/><circle cx="12.8" cy="8" r="1.35" fill="currentColor"/>"#,
};
/// The group menu's button, Finder's: a rule over three items, a rule, three more (measured
/// from Finder at 17.75 × 17 pt; drawn at 18 pt).
pub static GROUP: Icon = Icon {
    name: "rf-group",
    view: 18.0,
    body: r#"<path d="M0.7 0.7H17.3M0.7 10.1H17.3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="0.8" y="3.95" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="7.15" y="3.95" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="13.5" y="3.95" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="0.8" y="13.2" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="7.15" y="13.2" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><rect x="13.5" y="13.2" width="3.8" height="3.1" rx="1.1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
/// The toolbar's search button, Finder's weight: a 10.4 pt lens and handle in a 1.6 pt stroke
/// at 18 pt (the kit's search glyph is drawn for fields, thinner).
pub static SEARCH: Icon = Icon {
    name: "rf-search",
    view: 16.0,
    body: r#"<circle cx="6.9" cy="6.9" r="4.6" stroke="currentColor" stroke-width="1.45" fill="none"/><path d="M10.3 10.3L14.2 14.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
};
/// A sidebar favorite that isn't one of Finder's own folders.
pub static SIDEBAR_FOLDER: Icon = Icon {
    name: "rf-folder",
    view: 16.0,
    body: r#"<path d="M1.8 4A1.2 1.2 0 0 1 3 2.8H6.2L7.6 4.3H13A1.2 1.2 0 0 1 14.2 5.5V12.2A1.2 1.2 0 0 1 13 13.4H3A1.2 1.2 0 0 1 1.8 12.2ZM1.8 6.3H14.2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};

/// An external disk in the sidebar's 위치, as Finder draws it.
pub static EXTERNAL_DISK: Icon = Icon {
    name: "rf-external-disk",
    view: 16.0,
    body: r#"<rect x="1.5" y="4.5" width="13" height="7.5" rx="2" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" fill="none"/><path d="M1.5 9.2H14.5" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none"/><circle cx="12.2" cy="10.6" r="0.7" fill="currentColor"/>"#,
};
/// The list's disclosure chevrons (drawn at 16 pt, Finder's size: › 5 × 6.4, ⌄ 8 × 4.5) and the
/// group button's small one.
pub static CHEVRON_RIGHT: Icon = Icon {
    name: "rf-chevron-right",
    view: 16.0,
    body: r#"<path d="M5.6 4.8L10.5 8L5.6 11.2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
pub static CHEVRON_DOWN: Icon = Icon {
    name: "rf-chevron-down",
    view: 16.0,
    body: r#"<path d="M4 5.9L8 10.2L12 5.9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
/// The group button's chevron: 7 × 3.5 pt at 8 pt, heavier than the list's.
pub static GROUP_CHEVRON: Icon = Icon {
    name: "rf-group-chevron",
    view: 8.0,
    body: r#"<path d="M0.6 2.2L4 5.6L7.4 2.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};
