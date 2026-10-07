//! Keep the owned helper from surviving its parent; never assign inspected processes.
use super::{Handle, Result, io};
use std::{mem::size_of, os::windows::io::AsRawHandle, process::Child, ptr::null};
use windows_sys::Win32::System::JobObjects::*;

pub(crate) struct Job {
    _handle: Handle,
}
impl Job {
    pub(crate) fn attach(child: &Child) -> Result<Self> {
        // SAFETY: unnamed job, default security, owned returned handle.
        let job = Handle::new(
            unsafe { CreateJobObjectW(null(), null()) },
            "create inspection helper job",
        )?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: exact SDK structure/length, writable owned job; no inspected process belongs to it.
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io(
                "configure inspection helper job",
                std::io::Error::last_os_error(),
            ));
        }
        // SAFETY: Child owns its new process handle; job and child remain alive.
        if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
            return Err(io(
                "assign inspection helper job",
                std::io::Error::last_os_error(),
            ));
        }
        Ok(Self { _handle: job })
    }
}
