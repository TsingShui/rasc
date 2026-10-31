use std::cell::Cell;

use diana_decomp::input::{InputError, InputProblem, read_header};
use diana_decomp::{ControlError, FailureKind, Limits, RequestLimit, RunContext};

const HEADER_BYTES: usize = 0x70;
const FILE_SIZE_FIELD: usize = 0x20;
const HEADER_SIZE_FIELD: usize = 0x24;
const ENDIAN_FIELD: usize = 0x28;
const MAP_OFFSET_FIELD: usize = 0x34;
const TYPE_COUNT_FIELD: usize = 0x40;
const TYPE_OFFSET_FIELD: usize = 0x44;
const DATA_SIZE_FIELD: usize = 0x68;
const DATA_OFFSET_FIELD: usize = 0x6c;

fn put_u32(bytes: &mut [u8], offset_byte: usize, value: u32) {
    bytes[offset_byte..offset_byte + 4].copy_from_slice(&value.to_le_bytes());
}

// An empty 035 DEX: header followed by a two-record map (header and map_list).
fn empty_dex() -> Vec<u8> {
    let mut bytes = vec![0; HEADER_BYTES + 4 + 2 * 12];
    bytes[..8].copy_from_slice(b"dex\n035\0");
    let file_size = bytes.len() as u32;
    put_u32(&mut bytes, FILE_SIZE_FIELD, file_size);
    put_u32(&mut bytes, HEADER_SIZE_FIELD, HEADER_BYTES as u32);
    put_u32(&mut bytes, ENDIAN_FIELD, 0x1234_5678);
    put_u32(&mut bytes, MAP_OFFSET_FIELD, HEADER_BYTES as u32);
    put_u32(&mut bytes, DATA_SIZE_FIELD, file_size - HEADER_BYTES as u32);
    put_u32(&mut bytes, DATA_OFFSET_FIELD, HEADER_BYTES as u32);
    put_u32(&mut bytes, HEADER_BYTES, 2);
    put_u32(&mut bytes, HEADER_BYTES + 8, 1);
    bytes[HEADER_BYTES + 16..HEADER_BYTES + 18].copy_from_slice(&0x1000u16.to_le_bytes());
    put_u32(&mut bytes, HEADER_BYTES + 20, 1);
    put_u32(&mut bytes, HEADER_BYTES + 24, HEADER_BYTES as u32);
    bytes
}

fn empty_class_dex() -> Vec<u8> {
    let string_ids = HEADER_BYTES;
    let type_ids = string_ids + 8;
    let class_defs = type_ids + 8;
    let data = class_defs + 32;
    let mut bytes = vec![0; data + 1];
    bytes[..HEADER_BYTES].copy_from_slice(&empty_dex()[..HEADER_BYTES]);
    let mut strings = Vec::new();
    for text in ["Lfixtures/Empty;", "Ljava/lang/Object;"] {
        strings.push(bytes.len());
        bytes.push(text.len() as u8);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
    }
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let map = bytes.len();
    let entries = [
        (0u16, 1u32, 0usize),
        (1, 2, string_ids),
        (2, 2, type_ids),
        (6, 1, class_defs),
        (0x2002, 2, strings[0]),
        (0x1000, 1, map),
    ];
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (kind, count, offset) in entries {
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
    }
    for (field, value) in [
        (FILE_SIZE_FIELD, bytes.len()),
        (0x38, 2),
        (0x3c, string_ids),
        (TYPE_COUNT_FIELD, 2),
        (TYPE_OFFSET_FIELD, type_ids),
        (0x60, 1),
        (0x64, class_defs),
        (DATA_SIZE_FIELD, bytes.len() - data),
        (DATA_OFFSET_FIELD, data),
        (MAP_OFFSET_FIELD, map),
        (string_ids, strings[0]),
        (string_ids + 4, strings[1]),
        (type_ids + 4, 1),
        (class_defs + 4, 1),
        (class_defs + 8, 1),
        (class_defs + 16, u32::MAX as usize),
    ] {
        put_u32(&mut bytes, field, value as u32);
    }
    bytes
}

fn malformed(bytes: &[u8], problem: InputProblem) {
    let error = read_header(bytes, &mut RunContext::new(Limits::default(), &|| false))
        .expect_err("malformed header must be rejected");
    assert!(
        matches!(error, InputError::Malformed { code, .. } if code == problem),
        "{error}"
    );
    assert_eq!(error.kind(), FailureKind::Malformed);
}

#[test]
fn t02_empty_header_preserves_original_integrity_fields_without_authentication() {
    let mut bytes = empty_dex();
    put_u32(&mut bytes, 8, 0x8765_4321);
    bytes[12..32].fill(0xab);
    let mut control = RunContext::new(
        Limits {
            max_request_allocation_bytes: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    let header = read_header(&bytes, &mut control).unwrap();
    assert_eq!(header.checksum, 0x8765_4321);
    assert_eq!(header.signature, [0xab; 20]);
    assert_eq!(header.data_byte_range.as_range(), HEADER_BYTES..bytes.len());
    assert_eq!(header.map_byte_range.as_range(), HEADER_BYTES..bytes.len());
}

#[test]
fn t02_nonempty_tables_and_byte_aligned_string_items() {
    let bytes = empty_class_dex();
    let header = read_header(&bytes, &mut RunContext::new(Limits::default(), &|| false)).unwrap();
    assert_eq!(header.string_ids.count, 2);
    assert_eq!(header.type_ids.count, 2);
    assert_eq!(header.class_defs.count, 1);
    assert_eq!(
        header
            .class_defs
            .offset_byte
            .checked_advance(header.class_defs.size_byte)
            .unwrap(),
        header.data_byte_range.start
    );
    let mut overlap = bytes.clone();
    put_u32(&mut overlap, TYPE_OFFSET_FIELD, HEADER_BYTES as u32);
    malformed(&overlap, InputProblem::OverlappingSections);
    let mut conflict = bytes.clone();
    let map_offset = header.map_byte_range.start.0;
    put_u32(&mut conflict, map_offset + 4 + 2 * 12 + 4, 1);
    malformed(&conflict, InputProblem::InvalidMap);
}

#[test]
fn t02_header_truncation_and_declared_size_mismatch() {
    let bytes = empty_dex();
    for end in 0..HEADER_BYTES {
        malformed(&bytes[..end], InputProblem::Truncated);
    }
    let mut bad = bytes.clone();
    put_u32(&mut bad, FILE_SIZE_FIELD, bytes.len() as u32 + 1);
    malformed(&bad, InputProblem::FileSizeMismatch);
    put_u32(&mut bad, FILE_SIZE_FIELD, bytes.len() as u32 - 1);
    malformed(&bad, InputProblem::FileSizeMismatch);
    put_u32(&mut bad, FILE_SIZE_FIELD, bytes.len() as u32);
    put_u32(&mut bad, HEADER_SIZE_FIELD, 0x78);
    malformed(&bad, InputProblem::InvalidHeaderSize);
}

#[test]
fn t02_unknown_version_and_reverse_endian_are_unsupported() {
    let mut bytes = empty_dex();
    bytes[4..7].copy_from_slice(b"036");
    let error =
        read_header(&bytes, &mut RunContext::new(Limits::default(), &|| false)).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    bytes[4..7].copy_from_slice(b"035");
    put_u32(&mut bytes, ENDIAN_FIELD, 0x7856_3412);
    let error =
        read_header(&bytes, &mut RunContext::new(Limits::default(), &|| false)).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    put_u32(&mut bytes, ENDIAN_FIELD, 0);
    malformed(&bytes, InputProblem::InvalidEndianTag);
}

#[test]
fn t02_fixed_tables_are_bounded_aligned_and_outside_the_header() {
    let mut bytes = empty_dex();
    put_u32(&mut bytes, TYPE_COUNT_FIELD, 1);
    malformed(&bytes, InputProblem::InvalidSectionPair);
    put_u32(&mut bytes, TYPE_OFFSET_FIELD, HEADER_BYTES as u32 + 1);
    malformed(&bytes, InputProblem::InvalidAlignment);
    put_u32(&mut bytes, TYPE_OFFSET_FIELD, 0x7fff_fffc);
    malformed(&bytes, InputProblem::Truncated);
    put_u32(&mut bytes, TYPE_OFFSET_FIELD, 4);
    malformed(&bytes, InputProblem::OverlappingSections);
}

#[test]
fn t02_map_envelope_and_fixed_record_consistency() {
    let mut bytes = empty_dex();
    put_u32(&mut bytes, HEADER_BYTES, 0x0fff_ffff);
    malformed(&bytes, InputProblem::Truncated);
    let mut bytes = empty_dex();
    put_u32(&mut bytes, HEADER_BYTES + 24, HEADER_BYTES as u32 + 4);
    malformed(&bytes, InputProblem::InvalidMap);
    let mut bytes = empty_dex();
    bytes[HEADER_BYTES + 16..HEADER_BYTES + 18].fill(0);
    malformed(&bytes, InputProblem::InvalidMap);
    let mut bytes = empty_dex();
    bytes[HEADER_BYTES + 6] = 1;
    malformed(&bytes, InputProblem::InvalidMap);
}

#[test]
fn t02_malformed_count_is_checked_before_its_metadata_quota() {
    let mut bytes = empty_dex();
    put_u32(&mut bytes, HEADER_BYTES, 0x0fff_ffff);
    let limits = Limits {
        max_metadata_items: Some(0),
        ..Limits::default()
    };
    let error = read_header(&bytes, &mut RunContext::new(limits, &|| false)).unwrap_err();
    assert!(matches!(
        error,
        InputError::Malformed {
            code: InputProblem::Truncated,
            ..
        }
    ));
}

#[test]
fn t02_real_header_request_work_boundary_and_no_refund() {
    let bytes = empty_dex();
    let mut baseline = RunContext::new(Limits::default(), &|| false);
    read_header(&bytes, &mut baseline).unwrap();
    let work = baseline.usage().work_steps;
    let limits = Limits {
        max_request_work_steps: Some(work),
        ..Limits::default()
    };
    let mut control = RunContext::new(limits, &|| false);
    read_header(&bytes, &mut control).unwrap();
    assert!(matches!(
        read_header(&bytes, &mut control),
        Err(InputError::Control(ControlError::BudgetExceeded {
            limit: RequestLimit::MaxRequestWorkSteps,
            ..
        }))
    ));
    let limits = Limits {
        max_request_work_steps: Some(work - 1),
        ..limits
    };
    let mut control = RunContext::new(limits, &|| false);
    let error = read_header(&bytes, &mut control).unwrap_err();
    assert_eq!(error.kind(), FailureKind::BudgetExceeded);
    assert_eq!(
        read_header(&bytes, &mut control).unwrap_err().kind(),
        FailureKind::BudgetExceeded
    );
}

#[test]
fn t02_image_and_cumulative_metadata_limits() {
    let bytes = empty_dex();
    let limits = Limits {
        max_image_bytes: Some(bytes.len() as u64),
        max_metadata_items: Some(2),
        ..Limits::default()
    };
    let mut control = RunContext::new(limits, &|| false);
    read_header(&bytes, &mut control).unwrap();
    assert!(matches!(
        read_header(&bytes, &mut control),
        Err(InputError::Control(ControlError::BudgetExceeded {
            limit: RequestLimit::MaxMetadataItems,
            ..
        }))
    ));
    let limits = Limits {
        max_image_bytes: Some(bytes.len() as u64 - 1),
        ..Limits::default()
    };
    assert!(matches!(
        read_header(&bytes, &mut RunContext::new(limits, &|| false)),
        Err(InputError::Control(ControlError::BudgetExceeded {
            limit: RequestLimit::MaxImageBytes,
            ..
        }))
    ));
}

#[test]
fn t02_cancellation_during_header_work_remains_terminal() {
    for (bytes, cancel_after) in [(empty_dex(), 8), (super::container::d065(), 14)] {
        let checks = Cell::new(0);
        let cancel = || {
            checks.set(checks.get() + 1);
            checks.get() == cancel_after
        };
        let mut control = RunContext::new(Limits::default(), &cancel);
        let error = read_header(&bytes, &mut control).unwrap_err();
        assert_eq!(error.kind(), FailureKind::Cancelled);
        assert_eq!(
            read_header(&bytes, &mut control).unwrap_err().kind(),
            FailureKind::Cancelled
        );
    }
}
