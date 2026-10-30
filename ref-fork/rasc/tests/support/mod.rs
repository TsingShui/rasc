//! Small self-made binary fixtures for public stdio integration tests.
#![allow(dead_code)]

fn write_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn push_uleb(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

/// A valid DEX in which every class has one method that loads "Authorization".
pub fn const_string_dex(class_count: usize) -> Vec<u8> {
    assert!(class_count > 0);
    const HEADER_SIZE: usize = 0x70;
    let mut strings = vec!["Authorization".to_owned()];
    for index in 0..class_count {
        strings.push(format!("LFixture{index};"));
    }
    for index in 0..class_count {
        strings.push(format!("m{index}"));
    }
    strings.insert(class_count + 1, "V".to_owned());
    let n_strings = strings.len();
    let n_types = class_count + 2;

    let string_ids_off = HEADER_SIZE;
    let type_ids_off = string_ids_off + n_strings * 4;
    let proto_ids_off = type_ids_off + n_types * 4;
    let method_ids_off = proto_ids_off + 12;
    let class_defs_off = method_ids_off + class_count * 8;
    let data_off = class_defs_off + class_count * 32;

    let mut data = Vec::new();
    let mut string_offsets = Vec::with_capacity(n_strings);
    for value in &strings {
        string_offsets.push(data_off + data.len());
        push_uleb(&mut data, value.len() as u32);
        data.extend_from_slice(value.as_bytes());
        data.push(0);
    }

    let mut class_data_offsets = Vec::with_capacity(class_count);
    for index in 0..class_count {
        while !(data_off + data.len()).is_multiple_of(4) {
            data.push(0);
        }
        let code_off = (data_off + data.len()) as u32;
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&3_u32.to_le_bytes());
        data.extend_from_slice(&0x001a_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0x000e_u16.to_le_bytes());

        class_data_offsets.push(data_off + data.len());
        data.push(0);
        data.push(0);
        data.push(1);
        data.push(0);
        push_uleb(&mut data, index as u32);
        push_uleb(&mut data, 0);
        push_uleb(&mut data, code_off);
    }

    let total = data_off + data.len();
    let mut out = vec![0_u8; total];
    out[..8].copy_from_slice(b"dex\n039\0");
    write_u32(&mut out, 0x20, total as u32);
    write_u32(&mut out, 0x24, HEADER_SIZE as u32);
    write_u32(&mut out, 0x28, 0x1234_5678);
    write_u32(&mut out, 0x38, n_strings as u32);
    write_u32(&mut out, 0x3c, string_ids_off as u32);
    write_u32(&mut out, 0x40, n_types as u32);
    write_u32(&mut out, 0x44, type_ids_off as u32);
    write_u32(&mut out, 0x48, 1);
    write_u32(&mut out, 0x4c, proto_ids_off as u32);
    write_u32(&mut out, 0x58, class_count as u32);
    write_u32(&mut out, 0x5c, method_ids_off as u32);
    write_u32(&mut out, 0x60, class_count as u32);
    write_u32(&mut out, 0x64, class_defs_off as u32);

    for (index, offset) in string_offsets.iter().enumerate() {
        write_u32(&mut out, string_ids_off + index * 4, *offset as u32);
    }
    write_u32(&mut out, type_ids_off, 0);
    write_u32(
        &mut out,
        type_ids_off + (class_count + 1) * 4,
        (1 + class_count) as u32,
    );
    write_u32(&mut out, proto_ids_off, (1 + class_count) as u32);
    write_u32(&mut out, proto_ids_off + 4, (class_count + 1) as u32);
    for (index, class_data_off) in class_data_offsets.iter().enumerate() {
        write_u32(&mut out, type_ids_off + (index + 1) * 4, (index + 1) as u32);
        let method = method_ids_off + index * 8;
        write_u16(&mut out, method, (index + 1) as u16);
        write_u32(&mut out, method + 4, (2 + class_count + index) as u32);
        let class_def = class_defs_off + index * 32;
        write_u32(&mut out, class_def, (index + 1) as u32);
        write_u32(&mut out, class_def + 8, u32::MAX);
        write_u32(&mut out, class_def + 16, u32::MAX);
        write_u32(&mut out, class_def + 24, *class_data_off as u32);
    }
    out[data_off..].copy_from_slice(&data);
    let checksum = adler32(&out[12..]);
    write_u32(&mut out, 0x08, checksum);
    out
}

/// A minimal stored ZIP. CRC fields are zero because rasc does not read them.
pub fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zip(entries, false)
}

/// A minimal deflated ZIP for inflate-cache integration and benchmark fixtures.
pub fn deflated_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zip(entries, true)
}

fn zip(entries: &[(&str, &[u8])], deflated: bool) -> Vec<u8> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    for (name, payload) in entries {
        let compressed = if deflated {
            let mut compressor = flate2::Compress::new(flate2::Compression::best(), false);
            let mut out = vec![0; payload.len() * 2 + 1024];
            compressor
                .compress(payload, &mut out, flate2::FlushCompress::Finish)
                .unwrap();
            out.truncate(compressor.total_out() as usize);
            out
        } else {
            payload.to_vec()
        };
        let method = if deflated { 8u16 } else { 0u16 };
        let offset = local.len() as u32;
        local.extend_from_slice(b"PK\x03\x04");
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&method.to_le_bytes());
        local.extend_from_slice(&[0; 4]);
        local.extend_from_slice(&0u32.to_le_bytes());
        local.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        local.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        local.extend_from_slice(&(name.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name.as_bytes());
        local.extend_from_slice(&compressed);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&[0; 4]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = local.len() as u32;
    let central_size = central.len() as u32;
    local.extend_from_slice(&central);
    local.extend_from_slice(b"PK\x05\x06");
    local.extend_from_slice(&[0; 4]);
    local.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    local.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    local.extend_from_slice(&central_size.to_le_bytes());
    local.extend_from_slice(&central_offset.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local
}
