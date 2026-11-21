//! macOS: the Finder icon NSWorkspace shows for an application or binary,
//! rendered at 32 pt and encoded as PNG.

use crate::Icon;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_foundation::{NSDictionary, NSSize, NSString};
use std::path::{Component, Path, PathBuf};

pub(crate) fn icon(executable: &Path) -> Option<Icon> {
    // Prefer the enclosing .app bundle so the app icon, not a generic
    // "Unix executable" icon, is used for binaries inside Contents/MacOS.
    let target = bundle(executable).unwrap_or_else(|| executable.to_path_buf());
    let path = NSString::from_str(target.to_str()?);
    objc2::rc::autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let image = workspace.iconForFile(&path);
        image.setSize(NSSize::new(32.0, 32.0));
        let tiff = image.TIFFRepresentation()?;
        let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        Some(Icon::png(png.to_vec()))
    })
}

fn bundle(executable: &Path) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for component in executable.components() {
        path.push(component);
        if let Component::Normal(name) = component
            && name.to_string_lossy().ends_with(".app")
        {
            return Some(path);
        }
    }
    None
}
