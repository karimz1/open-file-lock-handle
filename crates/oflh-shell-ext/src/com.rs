//! COM glue: the `IExplorerCommand` handler, its class factory and the two
//! DLL exports Windows calls. Decisions live in [`crate::command`].

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOTIMPL, E_POINTER, HMODULE,
    S_FALSE, S_OK,
};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::Com::{
    CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl, IServiceProvider,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Ole::{IObjectWithSite, IObjectWithSite_Impl};
use windows::Win32::System::SystemServices::SFGAO_FOLDER;
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IFolderView, IShellBrowser,
    IShellItem, IShellItemArray, SHStrDupW, SIATTRIBFLAGS_AND, SID_STopLevelBrowser,
    SIGDN_FILESYSPATH,
};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref, implement};

use crate::command::{self, HANDLER_CLSID_U128, TargetKind};

// _EXPCMDSTATE and _EXPCMDFLAGS values.
const ECS_ENABLED: u32 = 0;
const ECS_HIDDEN: u32 = 2;
const ECF_DEFAULT: u32 = 0;

static LIVE_OBJECTS: AtomicUsize = AtomicUsize::new(0);

fn handler_clsid() -> GUID {
    GUID::from_u128(HANDLER_CLSID_U128)
}

/// Full path of this DLL.
fn this_dll() -> Option<PathBuf> {
    let mut module = HMODULE::default();
    let anchor = this_dll as *const c_void;
    // SAFETY: `anchor` is an address inside this DLL and `module` is a valid
    // out pointer; UNCHANGED_REFCOUNT means nothing has to be released.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(anchor.cast()),
            &mut module,
        )
        .ok()?;
    }
    let mut buffer = vec![0u16; 32_768];
    // SAFETY: `module` is this DLL's handle and the buffer is writable.
    let length = unsafe { GetModuleFileNameW(Some(module), &mut buffer) } as usize;
    (length > 0 && length < buffer.len())
        .then(|| PathBuf::from(OsString::from_wide(&buffer[..length])))
}

fn item_count(items: &Ref<'_, IShellItemArray>) -> usize {
    match items.as_ref() {
        // SAFETY: `items` is a live interface pointer for the call's duration.
        Some(items) => unsafe { items.GetCount() }.map_or(0, |count| count as usize),
        None => 0,
    }
}

fn target_kind(items: &Ref<'_, IShellItemArray>) -> TargetKind {
    let Some(items) = items.as_ref() else {
        return TargetKind::Folder;
    };
    // SAFETY: `items` is a live interface pointer for the call's duration.
    match unsafe { items.GetAttributes(SIATTRIBFLAGS_AND, SFGAO_FOLDER) } {
        Ok(attributes) if attributes.contains(SFGAO_FOLDER) => TargetKind::Folder,
        _ => TargetKind::File,
    }
}

fn first_path(items: &Ref<'_, IShellItemArray>) -> windows_core::Result<PathBuf> {
    let items = items.as_ref().ok_or(windows_core::Error::from(E_POINTER))?;
    // SAFETY: `items` is live for the call's duration.
    item_path(&unsafe { items.GetItemAt(0) }?)
}

fn item_path(item: &IShellItem) -> windows_core::Result<PathBuf> {
    // SAFETY: `item` is live; the returned string is CoTaskMem-allocated and
    // freed right after it is copied.
    unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(OsString::from_wide(name.as_wide()));
        CoTaskMemFree(Some(name.0.cast()));
        Ok(path)
    }
}

/// The folder whose background was right-clicked: Explorer passes no items
/// then, so ask the window hosting the menu (the command's site).
fn site_folder(site: &Mutex<Option<IUnknown>>) -> windows_core::Result<PathBuf> {
    let site = site
        .lock()
        .map_err(|_| windows_core::Error::from(E_FAIL))?
        .clone();
    let site = site.ok_or(windows_core::Error::from(E_FAIL))?;
    // SAFETY: COM calls on live interfaces owned by this function.
    unsafe {
        let provider: IServiceProvider = site.cast()?;
        let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser)?;
        let view: IFolderView = browser.QueryActiveShellView()?.cast()?;
        item_path(&view.GetFolder::<IShellItem>()?)
    }
}

fn co_task_string(text: &str) -> windows_core::Result<PWSTR> {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated; SHStrDupW copies it into CoTaskMem,
    // which Explorer frees.
    unsafe { SHStrDupW(PCWSTR(wide.as_ptr())) }
}

#[implement(IExplorerCommand, IObjectWithSite)]
struct InspectCommand {
    site: Mutex<Option<IUnknown>>,
}

impl InspectCommand {
    fn new() -> Self {
        LIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
        Self {
            site: Mutex::new(None),
        }
    }
}

impl IObjectWithSite_Impl for InspectCommand_Impl {
    fn SetSite(&self, site: Ref<'_, IUnknown>) -> windows_core::Result<()> {
        *self
            .site
            .lock()
            .map_err(|_| windows_core::Error::from(E_FAIL))? = site.cloned();
        Ok(())
    }

    fn GetSite(&self, iid: *const GUID, site: *mut *mut c_void) -> windows_core::Result<()> {
        if site.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        let current = self
            .site
            .lock()
            .map_err(|_| windows_core::Error::from(E_FAIL))?
            .clone();
        match current {
            // SAFETY: `iid` and `site` were checked for null; query writes `site`.
            Some(unknown) => unsafe { unknown.query(iid, site).ok() },
            None => {
                // SAFETY: `site` was checked for null.
                unsafe { *site = std::ptr::null_mut() };
                Err(E_FAIL.into())
            }
        }
    }
}

impl Drop for InspectCommand {
    fn drop(&mut self) {
        LIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IExplorerCommand_Impl for InspectCommand_Impl {
    fn GetTitle(&self, items: Ref<'_, IShellItemArray>) -> windows_core::Result<PWSTR> {
        // SAFETY: no arguments, no preconditions.
        let language = unsafe { GetUserDefaultUILanguage() };
        co_task_string(command::title(target_kind(&items), language))
    }

    fn GetIcon(&self, _items: Ref<'_, IShellItemArray>) -> windows_core::Result<PWSTR> {
        let app = this_dll()
            .and_then(|dll| command::app_path(&dll))
            .ok_or(windows_core::Error::from(E_FAIL))?;
        co_task_string(&format!("{},0", app.display()))
    }

    fn GetToolTip(&self, _items: Ref<'_, IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> windows_core::Result<GUID> {
        Ok(handler_clsid())
    }

    fn GetState(
        &self,
        items: Ref<'_, IShellItemArray>,
        _ok_to_be_slow: BOOL,
    ) -> windows_core::Result<u32> {
        let count = item_count(&items);
        let background = count == 0 && site_folder(&self.site).is_ok();
        Ok(if command::visible_for(count, background) {
            ECS_ENABLED
        } else {
            ECS_HIDDEN
        })
    }

    fn Invoke(
        &self,
        items: Ref<'_, IShellItemArray>,
        _bind: Ref<'_, IBindCtx>,
    ) -> windows_core::Result<()> {
        let target = if item_count(&items) == 0 {
            site_folder(&self.site)?
        } else {
            first_path(&items)?
        };
        let app = this_dll()
            .and_then(|dll| command::app_path(&dll))
            .ok_or(windows_core::Error::from(E_FAIL))?;
        std::process::Command::new(app)
            .args(command::inspect_args(&target))
            .spawn()
            .map(drop)
            .map_err(|_| E_FAIL.into())
    }

    fn GetFlags(&self) -> windows_core::Result<u32> {
        Ok(ECF_DEFAULT)
    }

    fn EnumSubCommands(&self) -> windows_core::Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<'_, IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> windows_core::Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: checked non-null above; COM callers pass valid pointers.
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = InspectCommand::new().into();
        // SAFETY: `iid` and `object` are valid per the checks above.
        unsafe { command.query(iid, object) }.ok()
    }

    fn LockServer(&self, _lock: BOOL) -> windows_core::Result<()> {
        Ok(())
    }
}

/// COM entry point: hands out the class factory for the handler CLSID.
///
/// # Safety
/// Called by COM with valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || iid.is_null() || object.is_null() {
        return E_POINTER;
    }
    // SAFETY: all three pointers were checked for null; COM passes valid ones.
    unsafe {
        *object = std::ptr::null_mut();
        if *clsid != handler_clsid() {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        factory.query(iid, object)
    }
}

/// COM asks whether the DLL can be unloaded.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_OBJECTS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
