//! What only macOS gives: Finder tags on files and the share services (AirDrop, Mail, Messages).
use std::path::{Path, PathBuf};

/// Finder's seven colour tags, in its menu's order, under the names a Korean Mac gives them.
pub const TAGS: [&str; 7] = ["빨간색", "주황색", "노란색", "초록색", "파란색", "보라색", "회색"];
const ENGLISH: [&str; 7] = ["Red", "Orange", "Yellow", "Green", "Blue", "Purple", "Gray"];

/// The colour index (into `TAGS`) of a tag's name, in Korean or English; None for a tag of the
/// user's own, which Finder draws without a colour.
pub fn tag_color(name: &str) -> Option<usize> {
    TAGS.iter().position(|t| *t == name).or_else(|| ENGLISH.iter().position(|t| *t == name))
}

/// Where share sends files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Share {
    AirDrop,
    Mail,
    Messages,
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSSharingService, NSSharingServiceNameComposeEmail, NSSharingServiceNameComposeMessage, NSSharingServiceNameSendViaAirDrop};
    use objc2_foundation::{NSArray, NSString, NSURL, NSURLTagNamesKey};
    use std::os::unix::ffi::OsStrExt;

    fn url(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    }

    /// Whether `path` carries any tags: one cheap xattr probe, so listing a folder only asks
    /// Foundation about the files that have some.
    fn has_tags(path: &Path) -> bool {
        let Ok(p) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
        let name = c"com.apple.metadata:_kMDItemUserTags";
        // SAFETY: both are valid C strings; a null buffer asks only for the value's size.
        let n = unsafe { libc::getxattr(p.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0, 0, libc::XATTR_NOFOLLOW) };
        n > 0
    }

    pub fn tags(path: &Path) -> Vec<String> {
        if !has_tags(path) {
            return vec![];
        }
        // Search reads tags on its own thread, which has no pool to drain Foundation's temporaries.
        objc2::rc::autoreleasepool(|_| {
            let mut value: Option<Retained<AnyObject>> = None;
            // SAFETY: NSURLTagNamesKey's value is an NSArray of NSString.
            if unsafe { url(path).getResourceValue_forKey_error(&mut value, NSURLTagNamesKey) }.is_err() {
                return vec![];
            }
            let Some(array) = value.and_then(|v| v.downcast::<NSArray>().ok()) else { return vec![] };
            array.iter().filter_map(|o| o.downcast::<NSString>().ok()).map(|s| s.to_string()).collect()
        })
    }

    pub fn set_tags(path: &Path, tags: &[String]) -> std::io::Result<()> {
        let names: Vec<Retained<NSString>> = tags.iter().map(|t| NSString::from_str(t)).collect();
        let array = NSArray::from_retained_slice(&names);
        // SAFETY: NSURLTagNamesKey takes an NSArray of NSString.
        unsafe { url(path).setResourceValue_forKey_error(Some(&array), NSURLTagNamesKey) }.map_err(|e| std::io::Error::other(e.localizedDescription().to_string()))
    }

    pub fn share(to: Share, paths: &[PathBuf]) -> Result<(), String> {
        // SAFETY: extern statics AppKit defines.
        let name = unsafe {
            match to {
                Share::AirDrop => NSSharingServiceNameSendViaAirDrop,
                Share::Mail => NSSharingServiceNameComposeEmail,
                Share::Messages => NSSharingServiceNameComposeMessage,
            }
        };
        let service = NSSharingService::sharingServiceNamed(name).ok_or("이 Mac에서 사용할 수 없는 공유 방법입니다.")?;
        let urls: Vec<Retained<AnyObject>> = paths.iter().map(|p| Retained::into_super(Retained::into_super(url(p)))).collect();
        let items: Retained<NSArray> = NSArray::from_retained_slice(&urls);
        // SAFETY: the items are file URLs, which these services accept.
        unsafe {
            if !service.canPerformWithItems(Some(&items)) {
                return Err("이 항목은 공유할 수 없습니다.".into());
            }
            service.performWithItems(&items);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    pub fn tags(_: &Path) -> Vec<String> {
        vec![]
    }
    pub fn set_tags(_: &Path, _: &[String]) -> std::io::Result<()> {
        Err(std::io::Error::other("태그는 macOS에서만 사용할 수 있습니다."))
    }
    pub fn share(_: Share, _: &[PathBuf]) -> Result<(), String> {
        Err("공유는 macOS에서만 사용할 수 있습니다.".into())
    }
}

pub use imp::{set_tags, share, tags};
