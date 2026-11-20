//! How Finder words sizes, dates and kinds (Korean locale).
use std::path::Path;

/// Finder counts in powers of 1000: "999바이트", "2KB", "1.5MB".
pub fn size(b: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if b < 1000 {
        return format!("{b}바이트");
    }
    let mut v = b as f64 / 1000.0;
    let mut u = 0;
    while v >= 1000.0 && u < 3 {
        v /= 1000.0;
        u += 1;
    }
    if v < 10.0 && u > 0 { format!("{v:.1}{}", UNITS[u]) } else { format!("{v:.0}{}", UNITS[u]) }
}

/// Free space as the status bar shows it: two decimals from a gigabyte up ("431.27GB").
pub fn free(b: u64) -> String {
    match b {
        1_000_000_000_000.. => format!("{:.2}TB", b as f64 / 1e12),
        1_000_000_000.. => format!("{:.2}GB", b as f64 / 1e9),
        _ => size(b),
    }
}

/// "1,234,567바이트", the exact count Get Info shows beside the rounded size.
pub fn bytes(b: u64) -> String {
    let digits = b.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{out}바이트")
}

/// Get Info's size: the rounded size and, from a kilobyte up, the exact count beside it.
pub fn size_detail(b: u64) -> String {
    if b < 1000 { size(b) } else { format!("{}({})", size(b), bytes(b)) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Local {
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    min: i32,
    /// Days since the epoch in local time, to tell today and yesterday apart.
    days: i64,
}

fn local(secs: i64) -> Local {
    // SAFETY: `tm` is plain data that `localtime_r` fills; both pointers are valid for the call.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&(secs as libc::time_t), &mut tm);
        tm
    };
    let days = (secs + tm.tm_gmtoff as i64).div_euclid(86_400);
    Local { year: tm.tm_year + 1900, month: tm.tm_mon + 1, day: tm.tm_mday, hour: tm.tm_hour, min: tm.tm_min, days }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Finder's list-view date: "오늘 오후 3:42", "어제 오전 9:05", else "2026년 11월 3일 오후 2:10".
pub fn date(secs: i64) -> String {
    date_at(secs, now())
}

pub fn date_at(secs: i64, now: i64) -> String {
    if secs == 0 {
        return "--".into();
    }
    let (t, today) = (local(secs), local(now));
    let h = t.hour % 12;
    let clock = format!("{} {}:{:02}", if t.hour < 12 { "오전" } else { "오후" }, if h == 0 { 12 } else { h }, t.min);
    match today.days - t.days {
        0 => format!("오늘 {clock}"),
        1 => format!("어제 {clock}"),
        _ => format!("{}년 {}월 {}일 {clock}", t.year, t.month, t.day),
    }
}

/// The kind column. Finder asks Launch Services; this covers the common types and falls back
/// to "<EXT> 문서" as Finder does for types no app claims.
pub fn kind(name: &str, dir: bool, link: bool) -> String {
    if link && !dir {
        return "별칭".into();
    }
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if dir {
        return match ext.as_deref() {
            Some("app") => "응용 프로그램",
            Some("bundle" | "framework") => "번들",
            Some("pages") => "Pages 문서",
            Some("numbers") => "Numbers 스프레드시트",
            Some("key") => "Keynote 프레젠테이션",
            _ => "폴더",
        }
        .into();
    }
    let Some(ext) = ext else { return "문서".into() };
    let known = match ext.as_str() {
        "png" => "PNG 이미지",
        "jpg" | "jpeg" => "JPEG 이미지",
        "gif" => "GIF 이미지",
        "heic" | "heif" => "HEIF 이미지",
        "tif" | "tiff" => "TIFF 이미지",
        "webp" => "WebP 이미지",
        "bmp" => "Windows BMP 이미지",
        "svg" => "SVG 문서",
        "pdf" => "PDF 문서",
        "txt" | "text" => "일반 텍스트 문서",
        "md" | "markdown" => "Markdown 문서",
        "rtf" => "RTF 문서",
        "html" | "htm" => "HTML 텍스트",
        "csv" => "쉼표로 구분된 값",
        "json" => "JSON",
        "xml" => "XML 텍스트",
        "zip" => "ZIP 압축",
        "gz" | "tgz" => "GZip 압축",
        "dmg" => "디스크 이미지",
        "pkg" => "설치 프로그램 패키지",
        "mp3" => "MP3 오디오",
        "m4a" => "Apple MPEG-4 오디오",
        "wav" => "Waveform 오디오",
        "aiff" | "aif" => "AIFF-C 오디오",
        "mov" => "QuickTime 동영상",
        "mp4" | "m4v" => "MPEG-4 동영상",
        "doc" | "docx" => "Microsoft Word 문서",
        "xls" | "xlsx" => "Microsoft Excel 통합 문서",
        "ppt" | "pptx" => "Microsoft PowerPoint 프레젠테이션",
        "sh" | "zsh" | "bash" => "셸 스크립트",
        "py" => "Python 스크립트",
        "js" | "mjs" => "JavaScript 스크립트",
        "ts" => "TypeScript 파일",
        "rs" => "Rust 소스",
        "swift" => "Swift 소스",
        "c" => "C 소스",
        "h" => "C 헤더 소스",
        "toml" => "TOML 문서",
        "yaml" | "yml" => "YAML 문서",
        _ => return format!("{} 문서", ext.to_uppercase()),
    };
    known.into()
}

/// What an item looks like, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Folder,
    App,
    Image,
    Text,
    Archive,
    Media,
    Document,
}

pub fn look(name: &str, dir: bool) -> Look {
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    match (dir, ext.as_deref()) {
        (true, Some("app")) => Look::App,
        (true, _) => Look::Folder,
        (_, Some("png" | "jpg" | "jpeg" | "gif" | "heic" | "heif" | "tif" | "tiff" | "webp" | "bmp" | "svg")) => Look::Image,
        (_, Some("txt" | "md" | "markdown" | "rtf" | "csv" | "json" | "xml" | "toml" | "yaml" | "yml" | "log")) => Look::Text,
        (_, Some("zip" | "gz" | "tgz" | "tar" | "dmg" | "pkg" | "7z" | "rar")) => Look::Archive,
        (_, Some("mp3" | "m4a" | "wav" | "aiff" | "aif" | "mov" | "mp4" | "m4v")) => Look::Media,
        _ => Look::Document,
    }
}
