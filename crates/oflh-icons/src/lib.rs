//! Application icons for process rows and termination confirmations.
//!
//! [`IconCache`] resolves an executable path to a small image once and keeps
//! the result (including "no icon") so repeated rows and refreshes cost a map
//! lookup. Native lookups are slow and may touch the file system, so callers
//! run [`IconCache::data_url`] off the UI thread. The UI shows a neutral
//! fallback icon whenever this returns `None`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod png;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::DesktopIndex;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Icons larger than this are ignored rather than sent to the webview.
pub const MAX_ICON_BYTES: usize = 256 * 1024;
/// Cached executables before the cache starts over.
pub const DEFAULT_CAPACITY: usize = 1024;

/// An encoded image ready for a `data:` URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    /// `image/png` or `image/svg+xml`.
    pub mime: &'static str,
    /// Encoded image bytes.
    pub bytes: Vec<u8>,
}

impl Icon {
    /// A PNG icon.
    pub fn png(bytes: Vec<u8>) -> Self {
        Self {
            mime: "image/png",
            bytes,
        }
    }
    /// `data:<mime>;base64,<bytes>`.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, base64(&self.bytes))
    }
}

type Loader = dyn Fn(&Path) -> Option<Icon> + Send + Sync;

/// Executable path -> icon, with negative results cached too.
pub struct IconCache {
    load: Box<Loader>,
    capacity: usize,
    entries: Mutex<HashMap<PathBuf, Option<Arc<str>>>>,
}

impl IconCache {
    /// A cache backed by the platform's icon lookup.
    pub fn native() -> Self {
        Self::with_loader(DEFAULT_CAPACITY, native_icon)
    }
    /// A cache backed by `load`; used by tests and other platforms.
    pub fn with_loader(
        capacity: usize,
        load: impl Fn(&Path) -> Option<Icon> + Send + Sync + 'static,
    ) -> Self {
        Self {
            load: Box::new(load),
            capacity: capacity.max(1),
            entries: Mutex::new(HashMap::new()),
        }
    }
    /// The icon for `executable` as a `data:` URL, or `None` for the fallback.
    ///
    /// Blocks while a native lookup runs; call it from a worker thread.
    pub fn data_url(&self, executable: &Path) -> Option<Arc<str>> {
        if executable.as_os_str().is_empty() {
            return None;
        }
        if let Some(hit) = self.lock().get(executable) {
            return hit.clone();
        }
        // Look up without holding the lock so other rows are not blocked.
        let url = (self.load)(executable)
            .filter(|icon| !icon.bytes.is_empty() && icon.bytes.len() <= MAX_ICON_BYTES)
            .map(|icon| Arc::<str>::from(icon.data_url()));
        let mut entries = self.lock();
        if entries.len() >= self.capacity {
            entries.clear();
        }
        entries
            .entry(executable.to_path_buf())
            .or_insert(url)
            .clone()
    }
    /// Number of cached executables (hits and misses).
    pub fn len(&self) -> usize {
        self.lock().len()
    }
    /// Whether nothing is cached yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, Option<Arc<str>>>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The platform's icon for `executable`, if it has one.
pub fn native_icon(executable: &Path) -> Option<Icon> {
    #[cfg(windows)]
    return windows::icon(executable);
    #[cfg(target_os = "macos")]
    return macos::icon(executable);
    #[cfg(target_os = "linux")]
    return linux::icon(executable);
    #[allow(unreachable_code)]
    {
        let _ = executable;
        None
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let n = (u32::from(group[0]) << 16)
            | (u32::from(*group.get(1).unwrap_or(&0)) << 8)
            | u32::from(*group.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if index <= group.len() {
                TABLE[(n >> shift) as usize & 63] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counting(icon: Option<Icon>) -> (Arc<AtomicUsize>, IconCache) {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let cache = IconCache::with_loader(3, move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
            icon.clone()
        });
        (calls, cache)
    }

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        for (input, output) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), output);
        }
    }

    #[test]
    fn caches_hits_per_executable() {
        let (calls, cache) = counting(Some(Icon::png(vec![1, 2, 3])));
        let first = cache.data_url(Path::new("/usr/bin/blender")).unwrap();
        assert_eq!(&*first, "data:image/png;base64,AQID");
        let again = cache.data_url(Path::new("/usr/bin/blender")).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        cache.data_url(Path::new("/usr/bin/node"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn caches_misses_so_the_fallback_is_not_retried_every_row() {
        let (calls, cache) = counting(None);
        assert!(cache.data_url(Path::new("/opt/tool")).is_none());
        assert!(cache.data_url(Path::new("/opt/tool")).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unknown_executables_use_the_fallback_without_a_lookup() {
        let (calls, cache) = counting(Some(Icon::png(vec![1])));
        assert!(cache.data_url(Path::new("")).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn empty_and_oversized_icons_fall_back() {
        let (_, empty) = counting(Some(Icon::png(Vec::new())));
        assert!(empty.data_url(Path::new("/a")).is_none());
        let (_, huge) = counting(Some(Icon::png(vec![0; MAX_ICON_BYTES + 1])));
        assert!(huge.data_url(Path::new("/a")).is_none());
    }

    #[test]
    fn stays_bounded() {
        let (calls, cache) = counting(None);
        for name in ["/a", "/b", "/c", "/d"] {
            cache.data_url(Path::new(name));
        }
        assert!(cache.len() <= 3);
        cache.data_url(Path::new("/d"));
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn is_shareable_across_worker_threads() {
        let (calls, cache) = counting(Some(Icon::png(vec![9])));
        let cache = Arc::new(cache);
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let cache = cache.clone();
                std::thread::spawn(move || cache.data_url(Path::new("/x")).unwrap())
            })
            .collect();
        let urls: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
        assert!(urls.iter().all(|url| **url == *urls[0]));
        assert!(calls.load(Ordering::SeqCst) <= 4);
        assert_eq!(cache.len(), 1);
    }
}
