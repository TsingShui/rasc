use super::input_facts::from_input;
use crate::input::DexContainer;
use crate::{ByteOffset, ClassDefId, FailureKind, Limits, RunContext};

const CONTAINER: &[u8] = include_bytes!("../../tests/fixtures/container/container.dex");

#[test]
fn lazy_declarations_keep_the_selected_logical_header() {
    for header in [0, 0x78] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let container = DexContainer::open(CONTAINER, &mut control).unwrap();
        let index = container.program(ByteOffset(header), &mut control).unwrap();
        let facts = from_input(&index, &mut control).unwrap();
        let descriptor: Vec<u16> = "Lfixtures/Case;".encode_utf16().collect();
        let class = facts.class(&descriptor, &mut control).unwrap().unwrap();
        assert_eq!(class.descriptor, descriptor);
        assert!(class.methods.iter().any(|method| {
            method.name == [u16::from(b'f')]
                && method.prototype == [u16::from(b'('), u16::from(b')'), u16::from(b'I')]
        }));
    }
}

#[test]
fn damaged_member_does_not_publish_a_complete_declaration_directory() {
    let mut bytes = CONTAINER.to_vec();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let container = DexContainer::open(&bytes, &mut control).unwrap();
    let index = container.program(ByteOffset(0x78), &mut control).unwrap();
    let name_offset = index.header.method_ids.offset_byte.0 + 4;
    bytes[name_offset..name_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());

    let container = DexContainer::open(&bytes, &mut control).unwrap();
    let index = container.program(ByteOffset(0x78), &mut control).unwrap();
    index.class(ClassDefId(0), &mut control).unwrap();
    let facts = from_input(&index, &mut control).unwrap();
    let descriptor: Vec<u16> = "Lfixtures/Case;".encode_utf16().collect();
    for _ in 0..2 {
        let class = facts.class(&descriptor, &mut control).unwrap().unwrap();
        assert!(matches!(
            class.coverage,
            crate::input::DeclarationCoverage::Partial
        ));
        assert_eq!(class.issues[0].kind(), FailureKind::Malformed);
        assert!(
            class
                .methods
                .iter()
                .any(|method| method.name == [u16::from(b'f')])
        );
    }
}
