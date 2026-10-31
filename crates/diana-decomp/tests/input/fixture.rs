pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) code: [usize; 2],
    pub(super) primary_method_idx: u32,
    pub(super) class_data: usize,
    pub(super) method_ids: usize,
    pub(super) handler_list: usize,
    pub(super) field_ids: usize,
    pub(super) proto_ids: usize,
    pub(super) literal_idx: u32,
    pub(super) literal_data: usize,
    pub(super) virtual_delta: usize,
}

pub(super) fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn uleb(bytes: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        bytes.push(byte | if value == 0 { 0 } else { 0x80 });
        if value == 0 {
            break;
        }
    }
}

fn align(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

pub(super) fn tiny(name: &str, protected: bool, huge_handler: bool) -> Fixture {
    build(name, protected, huge_handler, None)
}

pub(super) fn build(
    name: &str,
    protected: bool,
    huge_handler: bool,
    second_delta: Option<u32>,
) -> Fixture {
    build_internal(name, protected, huge_handler, second_delta, None)
}

#[derive(Clone, Copy)]
pub(super) struct CodeRecipe<'a> {
    pub(super) instructions: &'a [u16],
    pub(super) registers_size: u16,
    pub(super) outs_size: u16,
}

pub(super) fn with_code(recipe: CodeRecipe<'_>) -> Fixture {
    build_internal("f", false, false, None, Some(recipe))
}

fn build_internal(
    name: &str,
    protected: bool,
    huge_handler: bool,
    second_delta: Option<u32>,
    override_code: Option<CodeRecipe<'_>>,
) -> Fixture {
    let mut strings: Vec<Vec<u16>> = [
        "I",
        "J",
        "V",
        "II",
        "IJ",
        "Lfixtures/Case;",
        "Ljava/lang/Exception;",
        "Ljava/lang/Object;",
        "counter",
        "external",
        "g",
        name,
    ]
    .into_iter()
    .map(|s| s.encode_utf16().collect())
    .collect();
    strings.push(vec![0xd800, 0, 0xdc00]);
    strings.sort();
    strings.dedup();
    let literal_idx = strings
        .iter()
        .position(|s| s == &[0xd800, 0, 0xdc00])
        .unwrap() as u32;
    let sid = |text: &str| -> u32 {
        strings
            .iter()
            .position(|s| s.iter().copied().eq(text.encode_utf16()))
            .unwrap() as u32
    };
    let types = [
        "I",
        "J",
        "Lfixtures/Case;",
        "Ljava/lang/Exception;",
        "Ljava/lang/Object;",
        "V",
    ];
    let mut methods = [
        (2u16, 0u16, name),
        (2, 1, name),
        (2, 2, "g"),
        (4, 2, "external"),
    ];
    methods.sort_by_key(|&(owner, proto, name)| (owner, sid(name), proto));
    let direct: [u32; 2] = std::array::from_fn(|proto| {
        methods
            .iter()
            .position(|&(owner, p, _)| owner == 2 && usize::from(p) == proto)
            .unwrap() as u32
    });
    let virtual_idx = methods
        .iter()
        .position(|&(owner, proto, _)| owner == 2 && proto == 2)
        .unwrap() as u32;
    let string_ids = 0x70;
    let type_ids = string_ids + 4 * strings.len();
    let proto_ids = type_ids + types.len() * 4;
    let field_ids = proto_ids + 3 * 12;
    let method_ids = field_ids + 8;
    let class_defs = method_ids + 4 * 8;
    let data = class_defs + 32;
    let mut bytes = vec![0; data];
    bytes[..8].copy_from_slice(b"dex\n035\0");
    let params = bytes.len();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    align(&mut bytes);
    let long_params = bytes.len();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    align(&mut bytes);
    let mut code = [0; 2];
    let mut handler_list = 0;
    for (method, (registers, incoming, instructions)) in [
        (
            override_code.map_or(if protected { 2u16 } else { 1 }, |recipe| {
                recipe.registers_size
            }),
            1u16,
            if let Some(recipe) = override_code {
                recipe.instructions.to_vec()
            } else if protected {
                vec![0x0012u16, 0x0093, 0x0100, 0x000f, 0x000d, 0xf012, 0x000f]
            } else {
                vec![0x000f]
            },
        ),
        (3, 2, vec![0x0012u16, 0x000f]),
    ]
    .into_iter()
    .enumerate()
    {
        align(&mut bytes);
        code[method] = bytes.len();
        let outs = if method == 0 {
            override_code.map_or(0, |recipe| recipe.outs_size)
        } else {
            0
        };
        for word in [
            registers,
            incoming,
            outs,
            u16::from(protected && method == 0),
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(instructions.len() as u32).to_le_bytes());
        for word in instructions {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        if protected && method == 0 {
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&2u16.to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            handler_list = bytes.len();
            bytes.push(1);
            if huge_handler {
                bytes.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x07]);
            } else {
                bytes.push(1);
            }
            bytes.extend_from_slice(&[3, 4]);
        }
    }
    let class_data = bytes.len();
    bytes.extend_from_slice(&[1, 0, 2, 1, 0, 8]);
    for (delta, offset) in [
        (direct[0], code[0]),
        (second_delta.unwrap_or(direct[1] - direct[0]), code[1]),
    ] {
        uleb(&mut bytes, delta);
        uleb(&mut bytes, 9);
        uleb(&mut bytes, offset as u32);
    }
    let virtual_delta = bytes.len();
    uleb(&mut bytes, virtual_idx);
    uleb(&mut bytes, 0x401);
    uleb(&mut bytes, 0);
    let string_data = bytes.len();
    let mut literal_data = 0;
    for (index, units) in strings.iter().enumerate() {
        let start = bytes.len();
        if index == literal_idx as usize {
            literal_data = start;
        }
        put(&mut bytes, string_ids + index * 4, start as u32);
        uleb(&mut bytes, units.len() as u32);
        for &unit in units {
            match unit {
                1..=0x7f => bytes.push(unit as u8),
                0..=0x7ff => bytes
                    .extend_from_slice(&[(0xc0 | (unit >> 6)) as u8, (0x80 | (unit & 63)) as u8]),
                _ => bytes.extend_from_slice(&[
                    (0xe0 | (unit >> 12)) as u8,
                    (0x80 | ((unit >> 6) & 63)) as u8,
                    (0x80 | (unit & 63)) as u8,
                ]),
            }
        }
        bytes.push(0);
    }
    align(&mut bytes);
    let map = bytes.len();
    let entries = [
        (0u16, 1, 0),
        (1, strings.len(), string_ids),
        (2, types.len(), type_ids),
        (3, 3, proto_ids),
        (4, 1, field_ids),
        (5, 4, method_ids),
        (6, 1, class_defs),
        (0x1001, 2, params),
        (0x2001, 2, code[0]),
        (0x2000, 1, class_data),
        (0x2002, strings.len(), string_data),
        (0x1000, 1, map),
    ];
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (kind, count, offset) in entries {
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(count as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
    }
    for (index, descriptor) in types.into_iter().enumerate() {
        put(&mut bytes, type_ids + index * 4, sid(descriptor));
    }
    for (index, (shorty, ret, parameters)) in
        [("II", 0, params), ("IJ", 0, long_params), ("V", 5, 0)]
            .into_iter()
            .enumerate()
    {
        put(&mut bytes, proto_ids + index * 12, sid(shorty));
        put(&mut bytes, proto_ids + index * 12 + 4, ret);
        put(&mut bytes, proto_ids + index * 12 + 8, parameters as u32);
    }
    bytes[field_ids..field_ids + 2].copy_from_slice(&2u16.to_le_bytes());
    put(&mut bytes, field_ids + 4, sid("counter"));
    for (index, (owner, proto, method_name)) in methods.into_iter().enumerate() {
        bytes[method_ids + index * 8..method_ids + index * 8 + 2]
            .copy_from_slice(&owner.to_le_bytes());
        bytes[method_ids + index * 8 + 2..method_ids + index * 8 + 4]
            .copy_from_slice(&proto.to_le_bytes());
        put(&mut bytes, method_ids + index * 8 + 4, sid(method_name));
    }
    let size = bytes.len();
    for (field, value) in [
        (0x20, size),
        (0x24, 0x70),
        (0x28, 0x1234_5678),
        (0x34, map),
        (0x38, strings.len()),
        (0x3c, string_ids),
        (0x40, types.len()),
        (0x44, type_ids),
        (0x48, 3),
        (0x4c, proto_ids),
        (0x50, 1),
        (0x54, field_ids),
        (0x58, 4),
        (0x5c, method_ids),
        (0x60, 1),
        (0x64, class_defs),
        (0x68, size - data),
        (0x6c, data),
        (class_defs, 2),
        (class_defs + 4, 0x401),
        (class_defs + 8, 4),
        (class_defs + 16, u32::MAX as usize),
        (class_defs + 24, class_data),
    ] {
        put(&mut bytes, field, value as u32);
    }
    Fixture {
        bytes,
        code,
        primary_method_idx: direct[0],
        class_data,
        method_ids,
        handler_list,
        field_ids,
        proto_ids,
        literal_idx,
        literal_data,
        virtual_delta,
    }
}
