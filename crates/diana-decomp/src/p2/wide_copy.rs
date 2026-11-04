//! Test-only acceptance of captured word patches for DEX move-wide.
//! Verus checks the same body used by test replay; production excludes this module.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Clone, Copy)]
pub(super) struct CopyWord {
    pub(super) register: u32,
    pub(super) source: u32,
    pub(super) source_words: u32,
    pub(super) part: u8,
}

#[cfg(verus_keep_ghost)]
verus! {
    // The DEX rule reads both source words from the old frame, including overlap.
    pub(super) open spec fn dex_wide_at(old: Seq<u32>, source: int, destination: int, at: int) -> u32 {
        if at == destination { old[source] }
        else if at == destination + 1 { old[source + 1] }
        else { old[at] }
    }

    // P2 word writes denote captured old-slot inputs, not progressively read slots.
    pub(super) open spec fn patch_at(old: Seq<u32>, first: CopyWord, second: CopyWord, at: int) -> u32 {
        if at == second.register as int { old[second.source as int + second.part as int] }
        else if at == first.register as int { old[first.source as int + first.part as int] }
        else { old[at] }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(accepted =>
    ensures
        accepted == (
            source < u32::MAX && destination < u32::MAX
            && first.register as int == destination as int
            && second.register as int == destination as int + 1
            && first.source == source && second.source == source
            && first.source_words == 2 && second.source_words == 2
            && first.part == 0 && second.part == 1
        ),
        accepted ==> forall|old: Seq<u32>, at: int|
            source as int + 1 < old.len() && destination as int + 1 < old.len()
            && 0 <= at < old.len()
            ==> dex_wide_at(old, source as int, destination as int, at)
                == patch_at(old, first, second, at),
))]
pub(super) fn wide_copy_matches(
    source: u32,
    destination: u32,
    first: CopyWord,
    second: CopyWord,
) -> bool {
    if source == u32::MAX || destination == u32::MAX {
        return false;
    }
    first.register == destination
        && second.register == destination + 1
        && first.source == source
        && second.source == source
        && first.source_words == 2
        && second.source_words == 2
        && first.part == 0
        && second.part == 1
}
