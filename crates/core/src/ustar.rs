//! A reader of the ustar archive MakeMKV installs as appdata.tar, for its default.mmcp.xml (design part 5.4). Takes
//! bytes, never a path (RP19).

/// MakeMKV's default profile inside its appdata.tar (design part 5.4). Def: T4. Use: T23.
pub const DEFAULT_PROFILE_NAME: &str = "default.mmcp.xml";

/// The size of one tar block, which is also the size of a header.
const BLOCK: usize = 512;

/// The data of the first regular-file entry whose last path part equals `name`; Ok(None) when no entry has it; Err
/// for an archive cut short or a size field that is not octal. No I/O (RP19). Design part 5.4. Def: T4. Use: T23.
pub fn ustar_entry<'a>(archive: &'a [u8], name: &str) -> Result<Option<&'a [u8]>, String> {
    let mut offset = 0usize;
    loop {
        // The checks run in this order so that no slice below reaches past the end of a cut-short archive (PF-12).
        if offset >= archive.len() {
            return Ok(None);
        }
        let truncated = || format!("ustar: truncated entry at offset {offset}");
        let data_start = offset.checked_add(BLOCK).filter(|end| *end <= archive.len()).ok_or_else(truncated)?;
        let header = &archive[offset..data_start];
        if header.iter().all(|byte| *byte == 0) {
            return Ok(None);
        }
        let size = octal_size(&header[124..136]).ok_or_else(|| format!("ustar: bad size at offset {offset}"))?;
        let size = usize::try_from(size).map_err(|_| truncated())?;
        let data_end = data_start.checked_add(size).filter(|end| *end <= archive.len()).ok_or_else(truncated)?;
        let is_regular_file = header[156] == b'0' || header[156] == 0;
        let entry_name = header[..100].split(|byte| *byte == 0).next().unwrap_or(&[]);
        let last_part = entry_name.rsplit(|byte| *byte == b'/').next().unwrap_or(&[]);
        if is_regular_file && last_part == name.as_bytes() {
            return Ok(Some(&archive[data_start..data_end]));
        }
        // The data is padded with zeros to a whole block. The prefix field (offset 345) is not read: the last path
        // part always lies in the name field, and the checksum is not checked.
        offset = data_start + size.next_multiple_of(BLOCK);
    }
}

/// A header's size field: leading spaces, then one or more octal digits ended by a NUL, a space or the end of the
/// field. None for anything else.
fn octal_size(field: &[u8]) -> Option<u64> {
    let from = field.iter().position(|byte| *byte != b' ')?;
    let rest = &field[from..];
    let digits = &rest[..rest.iter().position(|byte| *byte == 0 || *byte == b' ').unwrap_or(rest.len())];
    if digits.is_empty() {
        return None;
    }
    let mut size = 0u64;
    for byte in digits {
        if !(b'0'..=b'7').contains(byte) {
            return None;
        }
        // At most 12 digits, so at most 36 bits: no overflow.
        size = size * 8 + u64::from(byte - b'0');
    }
    Some(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::ustar_build;

    #[test]
    fn the_default_profile_is_found_by_its_last_path_part() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(Some(&b"<a/>"[..])));
    }

    #[test]
    fn a_missing_entry_is_none() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, "other.xml"), Ok(None));
    }

    #[test]
    fn a_name_that_only_ends_with_the_wanted_name_does_not_match() {
        let archive = ustar_build(&[("x/my-default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(None));
    }

    #[test]
    fn the_first_of_two_matching_entries_is_returned() {
        let archive = ustar_build(&[("a/default.mmcp.xml", &b"1"[..]), ("b/default.mmcp.xml", &b"2"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(Some(&b"1"[..])));
    }

    #[test]
    fn a_longer_earlier_entry_is_skipped_to_the_next_header() {
        // The first header at 0, its 600-byte body rounded up to 1024, so the profile's header sits at 1536.
        let archive = ustar_build(&[("a/other.txt", &[b'x'; 600][..]), ("b/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Ok(Some(&b"<a/>"[..])));
    }

    #[test]
    fn a_truncated_archive_is_refused() {
        // An empty entry's header at 0, the profile's header at 512, its data at 1024.
        let archive = ustar_build(&[("x/empty.txt", &b""[..]), ("x/default.mmcp.xml", &b"<a/>"[..])]);
        let refused = Err("ustar: truncated entry at offset 512".to_string());
        assert_eq!(ustar_entry(&archive[..600], DEFAULT_PROFILE_NAME), refused);
        assert_eq!(ustar_entry(&archive[..1026], DEFAULT_PROFILE_NAME), refused);
    }

    #[test]
    fn a_bad_size_field_is_refused() {
        let mut archive = ustar_build(&[("default.mmcp.xml", &b"<a/>"[..])]);
        archive[124..136].copy_from_slice(b"zzzzzzzzzzz\0");
        assert_eq!(ustar_entry(&archive, DEFAULT_PROFILE_NAME), Err("ustar: bad size at offset 0".to_string()));
    }

    #[test]
    fn ustar_build_writes_one_header_one_data_block_and_two_zero_blocks() {
        let archive = ustar_build(&[("x/default.mmcp.xml", &b"<a/>"[..])]);
        assert_eq!(archive.len(), 4 * 512);
        assert_eq!(&archive[0..18], b"x/default.mmcp.xml");
        assert_eq!(&archive[124..136], b"00000000004\0");
        assert_eq!(&archive[156..157], b"0");
        assert_eq!(&archive[257..263], b"ustar\0");
        assert_eq!(&archive[512..516], b"<a/>");
        assert!(archive[1024..].iter().all(|byte| *byte == 0));
    }
}
