//! Validate initialized native descriptor records before setting vector lengths.
use oflh_core::{Error, Result};

/// Complete initialized records and conservative native coverage information.
pub(crate) struct DescriptorExtent {
    /// Records wholly initialized inside the supplied allocation.
    pub records: usize,
    /// A failed/empty read or full allocation cannot establish complete coverage.
    pub partial: bool,
}

/// Native byte counts must fit the allocation and contain complete ABI records.
pub(crate) fn descriptor_extent(
    returned: i32,
    capacity: usize,
    record_size: usize,
) -> Result<DescriptorExtent> {
    let capacity_bytes = capacity
        .checked_mul(record_size)
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| Error::Unavailable("invalid native descriptor buffer geometry".into()))?;
    if returned <= 0 {
        return Ok(DescriptorExtent {
            records: 0,
            partial: true,
        });
    }
    let returned = returned as usize;
    if returned > capacity_bytes || !returned.is_multiple_of(record_size) {
        return Err(Error::Unavailable(
            "invalid libproc descriptor length".into(),
        ));
    }
    Ok(DescriptorExtent {
        records: returned / record_size,
        partial: returned == capacity_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_complete_initialized_records_can_be_published() {
        for record_size in [4, 8, 16] {
            let extent = descriptor_extent((3 * record_size) as i32, 4096, record_size).unwrap();
            assert_eq!(extent.records, 3);
            assert!(!extent.partial);
            let extent = descriptor_extent((4096 * record_size) as i32, 4096, record_size).unwrap();
            assert_eq!(extent.records, 4096);
            assert!(extent.partial);
            for returned in [
                1,
                (3 * record_size - 1) as i32,
                (4096 * record_size + 1) as i32,
            ] {
                assert!(matches!(
                    descriptor_extent(returned, 4096, record_size),
                    Err(Error::Unavailable(_))
                ));
            }
        }
    }
    #[test]
    fn failed_reads_and_invalid_geometry_never_claim_initialized_memory() {
        for returned in [i32::MIN, -1, 0] {
            let extent = descriptor_extent(returned, 128, 8).unwrap();
            assert_eq!(extent.records, 0);
            assert!(extent.partial);
        }
        for (capacity, record_size) in [(0, 8), (128, 0), (usize::MAX, 8)] {
            assert!(matches!(
                descriptor_extent(1, capacity, record_size),
                Err(Error::Unavailable(_))
            ));
        }
    }
}
