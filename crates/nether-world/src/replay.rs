//! Answering from the ledger, and from nowhere else.

use std::collections::HashMap;

use nether_ledger::{Cairn, Call, Node, Store, Stored, Value};

/// What a trace already recorded, and nothing else.
///
/// §6.7: "The implementation MUST hold no capabilities at all during replay: it
/// does not prefer the ledger over the world, it cannot reach the world."
///
/// That is a claim about what this *is*, not about what it does. A `Replay`
/// holds a map from a question to what was said and nothing more — no
/// [`crate::World`], no [`crate::Provider`], no path, no socket — and there is
/// no method that takes one, so there is nothing here to reach the world with
/// and no way to hand it something that could.
///
/// Granting it anything does not compile:
///
/// ```compile_fail
/// # use nether_world::{Disk, Replay};
/// let replay = Replay::of(&[]);
/// let cheating = replay.granting(Box::new(Disk::reading(".")));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Replay {
    said: HashMap<Call, Vec<Value>>,
}

impl Replay {
    /// What those witnesses recorded.
    ///
    /// Takes the answers rather than the store they came from, because a
    /// `Replay` that kept a store could be asked to read one more thing.
    #[must_use]
    pub fn of(answers: &[(Call, Value)]) -> Self {
        let mut said: HashMap<Call, Vec<Value>> = HashMap::new();
        for (call, value) in answers {
            said.entry(call.clone()).or_default().push(value.clone());
        }
        Self { said }
    }

    /// Every witness a trace names, read out of the ledger.
    ///
    /// The store is used here and not kept: what comes back is a `Replay` that
    /// has already been told everything it will ever know.
    #[must_use]
    pub fn of_trace(store: &Store, witnesses: &[Cairn]) -> Self {
        // In the order the trace names them, because §6.3 merges a hole only
        // at a read stratum: one call can have been asked more than once, and
        // then the answers to it are told apart by nothing but their order.
        let mut said: HashMap<Call, Vec<Value>> = HashMap::new();
        for w in witnesses {
            if let Ok(Stored::Node(Node::Witness { call, answer, .. })) = store.get(*w)
                && let Ok(Stored::Value(v)) = store.get(answer)
            {
                said.entry(call).or_default().push(v);
            }
        }
        Self { said }
    }

    /// What was recorded for that question, if anything was.
    ///
    /// There is no fallback. §6.7 requires replay to fail rather than reach the
    /// world for an answer it does not have, and the way that is guaranteed
    /// here is that there is nowhere else to look.
    #[must_use]
    pub fn answer(&self, asked: &Call) -> Option<&Value> {
        self.said.get(asked)?.first()
    }

    /// Everything it was told, for handing to a burial.
    ///
    /// Each call once per answer recorded for it, in the order they were
    /// recorded. A burial takes them the same way (§6.3).
    pub fn all(&self) -> impl Iterator<Item = (&Call, &Value)> {
        self.said.iter().flat_map(|(call, values)| values.iter().map(move |v| (call, v)))
    }

    /// How many answers it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.said.values().map(Vec::len).sum()
    }

    /// Whether it holds none, in which case it can answer nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }
}
