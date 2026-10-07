//! Familiar Windows presentation without changing retained native action paths.
use std::path::Path;

/// Format a native path for display, sanitizing terminal control/formatting characters.
/// Ordinary Windows drive/UNC prefixes become familiar text; namespace-dependent
/// names retain their prefixes. The original native path must remain the action reference.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    let text = familiar_windows_path(&text, false);
    crate::safe(&text)
}

/// Format already lossless clipboard text while retaining namespace-dependent semantics.
/// Callers must reject non-Unicode paths rather than pass a lossy conversion.
/// On Windows, keep verbatim prefixes when the destination might require long-path opt-in.
pub fn clipboard_path_text(text: &str) -> String {
    #[cfg(windows)]
    {
        familiar_windows_path(text, true).into_owned()
    }
    #[cfg(not(windows))]
    {
        text.to_owned()
    }
}

#[cfg(any(windows, test))]
fn familiar_windows_path(text: &str, clipboard: bool) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let Some(native) = text.strip_prefix(r"\\?\") else {
        return Cow::Borrowed(text);
    };
    let (familiar, components) = if native.as_bytes().get(1..3) == Some(b":\\")
        && native
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
    {
        (Cow::Borrowed(native), &native[3..])
    } else if native
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"))
    {
        let components = &native[4..];
        let mut parts = components.split('\\');
        if !parts.next().is_some_and(|part| !part.is_empty())
            || !parts.next().is_some_and(|part| !part.is_empty())
        {
            return Cow::Borrowed(text);
        }
        (Cow::Owned(format!(r"\\{components}")), components)
    } else {
        return Cow::Borrowed(text);
    };
    // Win32 normalizes dots/spaces and interprets reserved names differently.
    // Keep verbatim clipboard paths at MAX_PATH, including the terminating NUL,
    // because the destination application's long-path opt-in is unknown.
    if !components.split('\\').all(ordinary_component)
        || (clipboard && familiar.encode_utf16().count() >= 260)
    {
        return Cow::Borrowed(text);
    }
    familiar
}

#[cfg(any(windows, test))]
fn ordinary_component(component: &str) -> bool {
    if component.ends_with([' ', '.'])
        || component
            .chars()
            .any(|character| character.is_control() || "<>:\"/|?*".contains(character))
    {
        return false;
    }
    let name = component.split('.').next().unwrap_or_default();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"]
        .iter()
        .any(|reserved| name.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    if name.get(..3).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT")
    }) {
        return !matches!(
            &name[3..],
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_roots_and_unc_paths_use_familiar_text() {
        for (native, expected) in [
            (r"\\?\C:\", r"C:\"),
            (r"\\?\d:\Projects\file ü.bin", r"d:\Projects\file ü.bin"),
            (r"\\?\UNC\server\share\file.bin", r"\\server\share\file.bin"),
            (r"\\?\UNC\server\share\", r"\\server\share\"),
            (r"C:\Projects\file.bin", r"C:\Projects\file.bin"),
            (r"/fixture/file\name", r"/fixture/file\name"),
        ] {
            assert_eq!(familiar_windows_path(native, false), expected);
            assert_eq!(familiar_windows_path(native, true), expected);
        }
    }

    #[test]
    fn namespace_dependent_paths_keep_their_original_meaning() {
        for native in [
            r"\\?\Volume{fixture}\file",
            r"\\?\GLOBALROOT\Device\fixture",
            r"\\.\PhysicalDrive0",
            r"\\?\C:relative",
            r"\\?\UNC\server",
            r"\\?\C:\file.",
            r"\\?\C:\folder \file",
            r"\\?\C:\..\file",
            r"\\?\C:\nul.txt",
            r"\\?\C:\COM¹.bin",
            r"\\?\C:\lpt9",
            r"\\?\C:\CONOUT$",
            r"\\?\C:\file/other",
            r"\\?\C:\file:stream",
        ] {
            assert_eq!(familiar_windows_path(native, false), native);
            assert_eq!(familiar_windows_path(native, true), native);
        }
    }

    #[test]
    fn long_clipboard_paths_keep_the_verbatim_prefix() {
        let native = format!(r"\\?\C:\{}", "ü".repeat(257));
        assert_eq!(familiar_windows_path(&native, false), &native[4..]);
        assert_eq!(familiar_windows_path(&native, true), native);
        let astral = format!(r"\\?\C:\{}", "😀".repeat(129));
        assert_eq!(familiar_windows_path(&astral, true), astral);
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_backslashes_are_not_windows_namespace_prefixes() {
        let text = r"\\?\C:\fixture";
        assert_eq!(display_path(Path::new(text)), text);
        assert_eq!(clipboard_path_text(text), text);
    }
}
