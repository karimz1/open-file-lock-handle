//! Windows: the shell's large icon for an executable (SHGetFileInfoW), read
//! back as 32-bit pixels and encoded as PNG.

use crate::{Icon, png};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

pub(crate) fn icon(executable: &Path) -> Option<Icon> {
    let wide: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: SHFILEINFOW is plain data; all-zero is a valid initial value.
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    // SAFETY: `wide` is NUL-terminated and outlives the call; `info` is a
    // writable SHFILEINFOW whose size is passed alongside it.
    let found = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            FILE_ATTRIBUTE_NORMAL,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    let pixels = icon_pixels(info.hIcon);
    // SAFETY: SHGFI_ICON transfers ownership of hIcon to the caller.
    unsafe { DestroyIcon(info.hIcon) };
    let (width, height, rgba) = pixels?;
    png::encode_rgba(width, height, &rgba).map(Icon::png)
}

fn icon_pixels(
    icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<(u32, u32, Vec<u8>)> {
    // SAFETY: ICONINFO is plain data; all-zero is a valid initial value.
    let mut parts: ICONINFO = unsafe { std::mem::zeroed() };
    // SAFETY: `icon` is a live icon handle and `parts` is writable.
    if unsafe { GetIconInfo(icon, &mut parts) } == 0 {
        return None;
    }
    let color = bitmap_bgra(parts.hbmColor);
    let mask = bitmap_bgra(parts.hbmMask);
    // SAFETY: GetIconInfo created both bitmaps for the caller to delete;
    // deleting a null handle is a harmless no-op.
    unsafe {
        DeleteObject(parts.hbmColor);
        DeleteObject(parts.hbmMask);
    }
    let (width, height, mut pixels) = color?;
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        pixel.swap(0, 2); // BGRA -> RGBA
        if !has_alpha {
            // Old icons without alpha: the AND mask is white where transparent.
            let opaque = mask
                .as_ref()
                .and_then(|(_, _, mask)| mask.get(index * 4))
                .is_none_or(|&value| value == 0);
            pixel[3] = if opaque { 255 } else { 0 };
        }
    }
    Some((width, height, pixels))
}

fn bitmap_bgra(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    if bitmap.is_null() {
        return None;
    }
    // SAFETY: BITMAP is plain data; all-zero is a valid initial value.
    let mut header: BITMAP = unsafe { std::mem::zeroed() };
    // SAFETY: `bitmap` is a live GDI bitmap and `header` matches the size passed.
    let read = unsafe {
        GetObjectW(
            bitmap,
            std::mem::size_of::<BITMAP>() as i32,
            (&raw mut header).cast(),
        )
    };
    let (width, height) = (header.bmWidth, header.bmHeight);
    if read == 0 || width <= 0 || height <= 0 || width > 256 || height > 256 {
        return None;
    }
    // SAFETY: BITMAPINFO is plain data; all-zero is a valid initial value.
    let mut format: BITMAPINFO = unsafe { std::mem::zeroed() };
    format.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height, // top-down rows
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..format.bmiHeader
    };
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    // SAFETY: a memory DC compatible with the screen; released below.
    let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return None;
    }
    // SAFETY: `pixels` holds exactly width*height 32-bit pixels as described
    // by `format`; `bitmap` is not selected into any DC.
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            pixels.as_mut_ptr().cast(),
            &mut format,
            DIB_RGB_COLORS,
        )
    };
    // SAFETY: `dc` was created above and is no longer used.
    unsafe { DeleteDC(dc) };
    (lines == height).then_some((width as u32, height as u32, pixels))
}
