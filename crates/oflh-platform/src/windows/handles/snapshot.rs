//! Class 64 ABI from phnt/ntexapi.h. Reject unsupported extents instead of truncating.
use super::super::*;
use std::mem::align_of;
use windows_sys::Wdk::System::SystemInformation::NtQuerySystemInformation;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct Entry {
    object: usize,
    pub process_id: usize,
    pub handle_value: usize,
    granted_access: u32,
    creator_trace: u16,
    pub object_type: u16,
    attributes: u32,
    reserved: u32,
}
const HEADER: usize = 2 * size_of::<usize>();

fn decode(buffer: &[usize], returned: usize) -> Result<Vec<Entry>> {
    if size_of::<usize>() != 8
        || size_of::<Entry>() != 40
        || align_of::<Entry>() > align_of::<usize>()
    {
        return Err(Error::Unavailable("unsupported native handle ABI".into()));
    }
    if returned < HEADER || returned > size_of_val(buffer) {
        return Err(Error::Unavailable(
            "invalid native handle snapshot extent".into(),
        ));
    }
    let count = buffer[0];
    if count > (returned - HEADER) / size_of::<Entry>() {
        return Err(Error::Unavailable(
            "invalid native handle snapshot count".into(),
        ));
    }
    // SAFETY: alignment/layout and initialized returned extent checked above.
    // Entries contain only integers; every bit pattern is valid. No kernel pointer
    // is dereferenced and no granted-access field is treated as fresh evidence.
    Ok(
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(2).cast::<Entry>(), count) }
            .to_vec(),
    )
}

pub(super) fn collect() -> Result<Vec<Entry>> {
    let mut buffer = vec![0usize; 128 * 1024 / size_of::<usize>()];
    loop {
        let bytes = size_of_val(buffer.as_slice());
        let mut returned = 0;
        // SAFETY: word-aligned writable allocation, exact bounded u32 length,
        // writable output. This undocumented ABI is confined to the helper;
        // failure yields an explicit partial/fallback warning in the caller.
        let status = unsafe {
            NtQuerySystemInformation(64, buffer.as_mut_ptr().cast(), bytes as u32, &mut returned)
        };
        if matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL
        ) {
            let next = (bytes * 2).max(returned as usize + 64 * 1024);
            if next > 128 * 1024 * 1024 {
                return Err(Error::Unavailable(
                    "native handle snapshot exceeds 128 MiB; no records truncated".into(),
                ));
            }
            buffer.resize(next.div_ceil(size_of::<usize>()), 0);
            continue;
        }
        if status != STATUS_SUCCESS {
            return Err(io(
                "snapshot native handles (NTSTATUS)",
                std::io::Error::from_raw_os_error(status),
            ));
        }
        return decode(&buffer, returned as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_and_returned_extents_are_checked() {
        assert_eq!(size_of::<Entry>(), 40);
        assert_eq!(align_of::<Entry>(), 8);
        let mut buffer = vec![0usize; 7];
        buffer[0] = 1;
        assert_eq!(decode(&buffer, 56).unwrap().len(), 1);
        for returned in [0, 15, 16, 55, 57, usize::MAX] {
            assert!(decode(&buffer, returned).is_err());
        }
        buffer[0] = usize::MAX;
        assert!(decode(&buffer, 56).is_err());
    }
}
