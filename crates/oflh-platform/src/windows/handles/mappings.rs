//! Enumerate data mappings independently of open file handles and loaded modules.
use super::super::*;
use super::{Context, Failures, NativeOperation, Reference, names::DevicePaths};
use crate::inspection_protocol::Failure;
use std::collections::HashSet;
use windows_sys::Win32::System::Memory::*;

fn mapped_path(process: &Handle, address: usize, devices: &DevicePaths) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 512];
    loop {
        // SAFETY: owned query-capable process; address is only passed to the OS,
        // never dereferenced locally. Output has the exact declared capacity.
        let length = unsafe {
            GetMappedFileNameW(
                process.0,
                address as *const _,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        } as usize;
        if length == 0 {
            return Err(io(
                "resolve mapped file path",
                std::io::Error::last_os_error(),
            ));
        }
        if length < buffer.len() {
            return devices.resolve(&buffer[..length]);
        }
        if buffer.len() >= 32_768 {
            return Err(Error::Unavailable(
                "mapped path exceeds helper extent".into(),
            ));
        }
        buffer.resize((buffer.len() * 2).min(32_768), 0);
    }
}

fn next_region(address: usize, base: usize, length: usize) -> Result<usize> {
    base.checked_add(length)
        .filter(|&next| length > 0 && base <= address && next > address)
        .ok_or_else(|| Error::Unavailable("invalid native memory region extent".into()))
}

pub(super) fn inspect(
    identity: Identity,
    pinned: &Handle,
    context: &Context<'_>,
    failures: &mut Failures,
) -> Result<()> {
    let _activity = context.operation(NativeOperation::Mapping);
    let process = match open(identity.pid, PROCESS_QUERY_INFORMATION | SYNCHRONIZE) {
        Ok(process) => process,
        Err(error) => {
            failures.note(Failure::Mapping, &error);
            return Ok(());
        }
    };
    if times(&process, identity.pid)?.0 != identity {
        return Err(Error::Changed);
    }
    let mut address = 0usize;
    let mut allocations = HashSet::new();
    loop {
        context.cancel.check()?;
        let mut info = MEMORY_BASIC_INFORMATION::default();
        context.stats.add(3, 1);
        // SAFETY: owned query handle, writable SDK output with exact native size;
        // remote address is a numeric query argument, never locally dereferenced.
        let returned = unsafe {
            VirtualQueryEx(
                process.0,
                address as *const _,
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if returned == 0 {
            let error = std::io::Error::last_os_error();
            if address > 0 && error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                break; // Above the process's highest accessible address.
            }
            return Err(io("enumerate process memory regions", error));
        }
        if returned != size_of::<MEMORY_BASIC_INFORMATION>() {
            return Err(Error::Unavailable(
                "invalid native memory information size".into(),
            ));
        }
        let next = next_region(address, info.BaseAddress as usize, info.RegionSize)?;
        if info.Type == MEM_MAPPED
            && info.State == MEM_COMMIT
            && allocations.insert(info.AllocationBase as usize)
        {
            context.stats.add(4, 1);
            match mapped_path(&process, info.AllocationBase as usize, context.devices) {
                Ok(path) if context.target.contains(&path) => {
                    context.publish(
                        identity,
                        pinned,
                        Reference {
                            path: &path,
                            directory: false,
                            mapped: true,
                            deleted: false,
                        },
                        failures,
                    )?;
                }
                Ok(_) => {}
                Err(error) => failures.note(Failure::Mapping, &error),
            }
        }
        address = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_memory_regions_cannot_loop_or_wrap() {
        assert_eq!(next_region(0x1100, 0x1000, 0x1000).unwrap(), 0x2000);
        for (address, base, length) in [
            (0, 0, 0),
            (0x1000, 0x1000, 0),
            (0x1000, 0x2000, 1),
            (usize::MAX - 1, usize::MAX - 1, 3),
        ] {
            assert!(next_region(address, base, length).is_err());
        }
    }
}
