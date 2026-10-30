//! What a trace says about itself, computed once.
//!
//! `spec/07-ledger.md` §7.3.2 gives a trace its derived facts, and three rites
//! write a trace. Working them out at each of those three is how each of them
//! came to be wrong at least once.

use nether_ledger::{Cairn, Node, Store, Stored};

/// The stratum a witness reached or a hole would reach, or 0 if it is neither.
fn reached(store: &Store, node: Cairn) -> u8 {
    match store.get(node) {
        Ok(Stored::Node(Node::Witness { stratum, .. } | Node::Hole { stratum, .. })) => stratum,
        _ => 0,
    }
}

/// §7.3.2: the join of what the residue still reaches, what a hole would
/// reach, and what a witness reached.
///
/// All three are needed. A stage-one burial has answered nothing, so the
/// witnesses say 0; a sealed trace has no holes left, so the holes say 0; and a
/// world-question demanded through a `seal` is a hole the residue does not
/// count, because §1.5 makes the seal pure.
pub fn depth(store: &Store, residue: u8, holes: &[Cairn], witnesses: &[Cairn]) -> u8 {
    holes.iter().chain(witnesses).map(|n| reached(store, *n)).fold(residue, u8::max)
}

/// §1.7: marked when something *reached* stratum 8, which a hole at 8 has not.
///
/// Not the same test as the depth. A trace whose only stratum-8 call is still a
/// hole has depth 8 and is not marked, because nothing has happened off the
/// record yet.
pub fn unrecorded(store: &Store, witnesses: &[Cairn]) -> bool {
    witnesses.iter().any(|w| reached(store, *w) == nether_ledger::MAX_STRATUM)
}

/// §6.8: what the trace deposited, then what burying its residue deposited.
///
/// Carried forward the way witnesses are. A deposit is something the program
/// already did, and a residue is right not to do it again — so a trace that
/// does not name the deposits of the one it came from loses them, and `nether
/// lamp` shows less after exhuming than before. In order, without repeating
/// one: a deposit is named by its content, so the same value at the same place
/// gives the same cairn, and naming it twice would say it happened twice.
pub fn deposits(before: &[Cairn], now: &[Cairn]) -> Vec<Cairn> {
    let mut out = before.to_vec();
    for one in now {
        if !out.contains(one) {
            out.push(*one);
        }
    }
    out
}
