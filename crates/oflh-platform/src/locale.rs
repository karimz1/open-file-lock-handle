//! Native locale fallback for terminal environments without locale variables.
/// Read the Windows user locale; Unix terminals conventionally supply LANG/LC_*.
/// Environment precedence is resolved by the caller. Failure retains the OS code.
pub fn system_locale() -> oflh_core::Result<Option<String>> {
    #[cfg(windows)]
    {
        let mut buffer = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH, including its terminator.
        // SAFETY: The writable UTF-16 buffer contains 85 initialized units, and the
        // API receives exactly that capacity. The returned extent is checked below.
        let written = unsafe {
            windows_sys::Win32::Globalization::GetUserDefaultLocaleName(buffer.as_mut_ptr(), 85)
        };
        if written == 0 {
            return Err(oflh_core::io(
                "read Windows user locale",
                std::io::Error::last_os_error(),
            ));
        }
        let length = usize::try_from(written)
            .ok()
            .filter(|&length| length > 1 && length <= buffer.len())
            .ok_or_else(|| oflh_core::Error::Unavailable("invalid Windows locale extent".into()))?;
        if buffer[length - 1] != 0 {
            return Err(oflh_core::Error::Unavailable(
                "unterminated Windows locale name".into(),
            ));
        }
        let locale = String::from_utf16(&buffer[..length - 1])
            .map_err(|_| oflh_core::Error::Unavailable("invalid Windows locale UTF-16".into()))?;
        Ok(Some(locale))
    }
    #[cfg(not(windows))]
    Ok(None)
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn native_user_locale_is_nonempty_and_lossless_unicode() {
        let locale = super::system_locale().unwrap().unwrap();
        assert!(!locale.is_empty());
        assert!(!locale.contains('\0'));
        assert!(locale.len() < 85);
        assert!(
            locale
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        );
    }
}
