//! Remote Desktop Services (WTS) sessions and SMB server open files.
use crate::{
    Error, Result, Session, ShareFile, Source,
    wts::{self, RawSession},
};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, GetLastError},
    NetworkManagement::NetManagement::{MAX_PREFERRED_LENGTH, NERR_Success, NetApiBufferFree},
    Storage::FileSystem::{
        FILE_INFO_3, NetFileClose, NetFileEnum, NetSessionEnum, PERM_FILE_WRITE, SESSION_INFO_10,
    },
    System::RemoteDesktop::{
        ProcessIdToSessionId, WTS_CURRENT_SERVER_HANDLE, WTS_INFO_CLASS, WTS_SESSION_INFOW,
        WTSClientName, WTSClientProtocolType, WTSDomainName, WTSEnumerateSessionsW, WTSFreeMemory,
        WTSINFOW, WTSLogoffSession, WTSQuerySessionInformationW, WTSSendMessageW, WTSSessionInfo,
        WTSUserName,
    },
};

pub(crate) struct Native;

const MB_OK: u32 = 0;
const MB_ICONINFORMATION: u32 = 0x40;
const ERROR_MORE_DATA: u32 = 234;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// # Safety
/// `ptr` must be null or a NUL-terminated UTF-16 string.
unsafe fn from_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: the caller guarantees NUL termination, so every read up to it is in bounds.
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` elements were just read from the same allocation.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, len) })
}

fn last_error(call: &str) -> Error {
    // SAFETY: GetLastError has no preconditions.
    let code = unsafe { GetLastError() };
    if code == ERROR_ACCESS_DENIED {
        Error::PermissionDenied
    } else {
        Error::Failed(format!("{call} failed (Windows error {code})"))
    }
}

/// Query one information class; the buffer is freed before returning.
fn query<T>(id: u32, class: WTS_INFO_CLASS, read: impl FnOnce(*const u8, u32) -> T) -> Option<T> {
    let mut buffer: windows_sys::core::PWSTR = null_mut();
    let mut bytes = 0u32;
    // SAFETY: out pointers are valid locals; the server handle constant is documented.
    let ok = unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            id,
            class,
            &mut buffer,
            &mut bytes,
        )
    };
    if ok == 0 || buffer.is_null() {
        return None;
    }
    let value = read(buffer.cast(), bytes);
    // SAFETY: the buffer was allocated by WTSQuerySessionInformationW.
    unsafe { WTSFreeMemory(buffer.cast()) };
    Some(value)
}

fn text(id: u32, class: WTS_INFO_CLASS) -> String {
    // SAFETY: string classes return a NUL-terminated UTF-16 buffer.
    query(id, class, |ptr, _| unsafe { from_wide(ptr.cast()) }).unwrap_or_default()
}

fn raw(info: &WTS_SESSION_INFOW) -> RawSession {
    let id = info.SessionId;
    let protocol = query(id, WTSClientProtocolType, |ptr, bytes| {
        // SAFETY: this class returns a USHORT; the size is checked first.
        (bytes as usize >= 2).then(|| unsafe { ptr.cast::<u16>().read_unaligned() })
    })
    .flatten()
    .unwrap_or(u16::MAX);
    let times = query(id, WTSSessionInfo, |ptr, bytes| {
        // SAFETY: this class returns one WTSINFOW; the size is checked first.
        (bytes as usize >= size_of::<WTSINFOW>()).then(|| unsafe {
            let info = ptr.cast::<WTSINFOW>().read_unaligned();
            (info.LogonTime, info.ConnectTime)
        })
    })
    .flatten()
    .unwrap_or_default();
    RawSession {
        id,
        // SAFETY: WTSEnumerateSessionsW returns NUL-terminated station names.
        station: unsafe { from_wide(info.pWinStationName) },
        state: info.State,
        user: text(id, WTSUserName),
        domain: text(id, WTSDomainName),
        client: text(id, WTSClientName),
        protocol,
        logon_time: times.0,
        connect_time: times.1,
    }
}

fn net_sessions() -> Vec<(String, String)> {
    let mut buffer: *mut u8 = null_mut();
    let (mut read, mut total) = (0u32, 0u32);
    // SAFETY: out pointers are valid; null server/client/user filters mean "all".
    let status = unsafe {
        NetSessionEnum(
            null(),
            null(),
            null(),
            10,
            &mut buffer,
            MAX_PREFERRED_LENGTH,
            &mut read,
            &mut total,
            null_mut(),
        )
    };
    let mut pairs = Vec::new();
    if (status == NERR_Success || status == ERROR_MORE_DATA) && !buffer.is_null() {
        // SAFETY: level 10 fills `read` SESSION_INFO_10 records in `buffer`.
        let items =
            unsafe { std::slice::from_raw_parts(buffer.cast::<SESSION_INFO_10>(), read as usize) };
        for item in items {
            // SAFETY: the record strings are NUL-terminated and live until the buffer is freed.
            pairs.push(unsafe {
                (
                    from_wide(item.sesi10_cname),
                    from_wide(item.sesi10_username),
                )
            });
        }
    }
    if !buffer.is_null() {
        // SAFETY: allocated by NetSessionEnum.
        unsafe { NetApiBufferFree(buffer.cast()) };
    }
    pairs
}

impl Source for Native {
    fn sessions(&self) -> Result<Vec<Session>> {
        let mut list: *mut WTS_SESSION_INFOW = null_mut();
        let mut count = 0u32;
        // SAFETY: out pointers are valid locals; reserved must be 0 and version 1.
        if unsafe { WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut list, &mut count) }
            == 0
        {
            return Err(last_error("WTSEnumerateSessionsW"));
        }
        // SAFETY: the call returned `count` records at `list`.
        let infos = unsafe { std::slice::from_raw_parts(list, count as usize) };
        let sessions = infos.iter().map(raw).filter_map(wts::map).collect();
        // SAFETY: allocated by WTSEnumerateSessionsW; `infos` is not used afterwards.
        unsafe { WTSFreeMemory(list.cast()) };
        Ok(sessions)
    }
    fn process_session(&self, pid: u32) -> Option<String> {
        let mut session = 0u32;
        // SAFETY: the out pointer is a valid local.
        (unsafe { ProcessIdToSessionId(pid, &mut session) } != 0).then(|| session.to_string())
    }
    fn share_files(&self) -> Result<Vec<ShareFile>> {
        let clients = net_sessions();
        let mut files = Vec::new();
        let mut resume = 0usize;
        loop {
            let mut buffer: *mut u8 = null_mut();
            let (mut read, mut total) = (0u32, 0u32);
            // SAFETY: out pointers are valid; null filters enumerate every open file.
            let status = unsafe {
                NetFileEnum(
                    null(),
                    null(),
                    null(),
                    3,
                    &mut buffer,
                    MAX_PREFERRED_LENGTH,
                    &mut read,
                    &mut total,
                    &mut resume,
                )
            };
            if status != NERR_Success && status != ERROR_MORE_DATA {
                if !buffer.is_null() {
                    // SAFETY: allocated by NetFileEnum.
                    unsafe { NetApiBufferFree(buffer.cast()) };
                }
                return Err(if status == ERROR_ACCESS_DENIED {
                    Error::PermissionDenied
                } else {
                    Error::Failed(format!("NetFileEnum failed (error {status})"))
                });
            }
            if !buffer.is_null() {
                // SAFETY: level 3 fills `read` FILE_INFO_3 records.
                let items = unsafe {
                    std::slice::from_raw_parts(buffer.cast::<FILE_INFO_3>(), read as usize)
                };
                for item in items {
                    // SAFETY: record strings are NUL-terminated within the buffer.
                    let (path, user) =
                        unsafe { (from_wide(item.fi3_pathname), from_wide(item.fi3_username)) };
                    files.push(ShareFile {
                        id: item.fi3_id,
                        client: wts::clients_for(&user, &clients),
                        path,
                        user,
                        locks: item.fi3_num_locks,
                        write: item.fi3_permissions & PERM_FILE_WRITE != 0,
                    });
                }
                // SAFETY: allocated by NetFileEnum; `items` is not used afterwards.
                unsafe { NetApiBufferFree(buffer.cast()) };
            }
            if status != ERROR_MORE_DATA {
                return Ok(files);
            }
        }
    }
    fn send_message(&self, session: &str, title: &str, text: &str) -> Result<()> {
        let id: u32 = session.parse().map_err(|_| Error::Changed)?;
        let (title, text) = (wide(title), wide(text));
        let mut response = 0i32;
        // SAFETY: both strings are NUL-terminated buffers alive for the call; byte
        // lengths exclude the terminator as documented. bWait = 0 returns at once.
        let ok = unsafe {
            WTSSendMessageW(
                WTS_CURRENT_SERVER_HANDLE,
                id,
                title.as_ptr(),
                ((title.len() - 1) * 2) as u32,
                text.as_ptr(),
                ((text.len() - 1) * 2) as u32,
                MB_OK | MB_ICONINFORMATION,
                0,
                &mut response,
                0,
            )
        };
        if ok == 0 {
            Err(last_error("WTSSendMessageW"))
        } else {
            Ok(())
        }
    }
    fn sign_out(&self, session: &str) -> Result<()> {
        let id: u32 = session.parse().map_err(|_| Error::Changed)?;
        // SAFETY: plain values; bWait = 0 returns after the request is queued.
        if unsafe { WTSLogoffSession(WTS_CURRENT_SERVER_HANDLE, id, 0) } == 0 {
            Err(last_error("WTSLogoffSession"))
        } else {
            Ok(())
        }
    }
    fn close_share_file(&self, id: u32) -> Result<()> {
        // SAFETY: null server means this computer; `id` is a plain value.
        match unsafe { NetFileClose(null(), id) } {
            status if status == NERR_Success => Ok(()),
            ERROR_ACCESS_DENIED => Err(Error::PermissionDenied),
            2314 /* NERR_FileIdNotFound */ => Err(Error::Changed),
            status => Err(Error::Failed(format!("NetFileClose failed (error {status})"))),
        }
    }
}
