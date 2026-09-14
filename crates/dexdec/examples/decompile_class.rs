//! Decompile one class from a bare DEX, Java only.
//!
//! Usage: cargo run --release -p dexdec --example decompile_class -- <dex> <descriptor>...
//!
//! This is the fork's smoke tool: the same `from_bytes` + `class()` path rasc
//! uses for `getclass --emitter dexdec`.

use dexdec::{DecompileOptions, Decompiler, SourceLanguage};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: decompile_class <dex> <descriptor>...");
        std::process::exit(2);
    }
    let bytes = std::fs::read(&args[0]).expect("read dex");
    let options = DecompileOptions::default().with_language(SourceLanguage::Java);
    let mut decompiler = Decompiler::from_bytes(&bytes)
        .expect("open dex")
        .with_options(options);
    let mut failed = 0;
    for name in &args[1..] {
        match decompiler.class(name.clone()) {
            Ok(unit) => {
                println!("// class: {name}");
                print!("{}", unit.source);
            }
            Err(err) => {
                eprintln!("error: {name}: {err}");
                failed += 1;
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
