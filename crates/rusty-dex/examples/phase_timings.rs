//! Per-phase parse timings for the metadata build (`build_metadata`), measured
//! phase by phase so a scoped parse has a budget to work against.
use rusty_dex::dex::classes::{ClassDecodeLevel, DexClasses};
use rusty_dex::dex::fields::DexFields;
use rusty_dex::dex::header::DexHeader;
use rusty_dex::dex::methods::DexMethods;
use rusty_dex::dex::protos::DexProtos;
use rusty_dex::dex::reader::DexReader;
use rusty_dex::dex::strings::DexStrings;
use rusty_dex::dex::types::DexTypes;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("usage: phase_timings <dex>");
    let bytes = std::fs::read(&path).expect("read dex");
    let mut t = Instant::now();
    let mut mark = |name: &str, t: &mut Instant| {
        eprintln!("[phase] {name:22} {:?}", t.elapsed());
        *t = Instant::now();
    };
    let mut r = DexReader::build(bytes).expect("reader");
    mark("reader(wrap bytes)", &mut t);
    let h = DexHeader::new(&mut r).expect("header");
    mark("header", &mut t);
    let s = DexStrings::build(&mut r, h.string_ids_off, h.string_ids_size).expect("strings");
    mark("strings", &mut t);
    let ty = DexTypes::build(&mut r, h.type_ids_off, h.type_ids_size, &s).expect("types");
    mark("types", &mut t);
    let p = DexProtos::build(&mut r, h.proto_ids_off, h.proto_ids_size, &ty).expect("protos");
    mark("protos", &mut t);
    let f = DexFields::build(&mut r, h.fields_ids_off, h.fields_ids_size, &ty, &s).expect("fields");
    mark("fields", &mut t);
    let m = DexMethods::build(&mut r, h.method_ids_off, h.method_ids_size, &ty, &p, &s)
        .expect("methods");
    mark("methods", &mut t);
    let c = DexClasses::build_with_level(
        &mut r, h.class_defs_off, h.class_defs_size, &f, &ty, &p, &s, &m,
        ClassDecodeLevel::Declaration,
    )
    .expect("classes");
    mark("classes(declaration)", &mut t);
    eprintln!(
        "[counts] strings={} types={} protos={} fields={} methods={} classes={}",
        s.strings.len(), ty.items.len(), p.items.len(), f.items.len(), m.items.len(), c.items.len()
    );
}
