//! Timing split for one class out of a bare DEX. `DEXDEC_PHASE_TIMING=1` adds the
//! parser's per-pool phases.
use dexdec::Decompiler;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&a[0]).expect("read dex");
    let t = Instant::now();
    let mut d = Decompiler::from_bytes(&bytes).expect("open");
    eprintln!("[total] open (parse): {:?}", t.elapsed());
    let t = Instant::now();
    let _ = d.class(a[1].clone()).expect("class 1");
    eprintln!("[total] first class: {:?}", t.elapsed());
    let t = Instant::now();
    let _ = d.class(a[2].clone()).expect("class 2");
    eprintln!("[total] second class: {:?}", t.elapsed());
}
