//! What a trace says about itself, computed once.
//!
//! `spec/07-ledger.md` §7.3.2 gives a trace two derived facts, and three rites
//! write a trace. Working them out at each of those three is how each of them
//! came to be wrong at least once.

use nether_ledger::{Cairn, Node, Store, Stored};

/// The stratum that witness reached, or 0 if it is not one.
fn reached(store: &Store, witness: Cairn) -> u8 {
    match store.get(witness) {
        Ok(Stored::Node(Node::Witness { stratum, .. })) => stratum,
        _ => 0,
    }
}

/// §7.3.2: the join of what the residue still reaches and what a witness
/// reached.
///
/// Both halves are needed. A stage-one burial has answered nothing, so the
/// witnesses say 0 and the residue says how deep the program still goes; a
/// sealed trace has no holes left, so the residue says 0 and the witnesses say
/// how deep it went.
pub fn depth(store: &Store, residue: u8, witnesses: &[Cairn]) -> u8 {
    witnesses.iter().map(|w| reached(store, *w)).fold(residue, u8::max)
}

/// §1.7: marked when something *reached* stratum 8, which a hole at 8 has not.
///
/// Not the same test as the depth. A trace whose only stratum-8 call is still a
/// hole has depth 8 and is not marked, because nothing has happened off the
/// record yet.
pub fn unrecorded(store: &Store, witnesses: &[Cairn]) -> bool {
    witnesses.iter().any(|w| reached(store, *w) == nether_ledger::MAX_STRATUM)
}
