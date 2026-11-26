use nether_core::{BinOp, Literal, Prim, Refusal as RefusalCode, Type, UnOp};
use nether_ledger::{Cairn, Stratum, Value};

use crate::residue::MAX_CONCAT;
use crate::state::{Kind, Val, ValLit};

pub(crate) fn unary(op: UnOp, v: &Val) -> Option<Literal> {
    Some(match op {
        // Arithmetic wraps. An implementation MUST NOT make overflow
        // undefined: a canonical encoding cannot be built on top of behaviour
        // that varies by compiler. `spec/05-types.md` §5.1.
        UnOp::Neg => Literal::Int(v.int()?.wrapping_neg()),
        UnOp::BitNot => Literal::Int(!v.int()?),
        UnOp::Not => Literal::Bool(!v.bool()?),
    })
}

pub(crate) fn binary(op: BinOp, a: &Val, b: &Val) -> Result<Option<Literal>, &'static str> {
    if let (Some(x), Some(y)) = (a.int(), b.int()) {
        return Ok(Some(match op {
            BinOp::Add => Literal::Int(x.wrapping_add(y)),
            BinOp::Sub => Literal::Int(x.wrapping_sub(y)),
            BinOp::Mul => Literal::Int(x.wrapping_mul(y)),
            // Dividing by zero cannot produce a value and never will.
            BinOp::Div | BinOp::Rem if y == 0 => return Err("this divides by zero"),
            BinOp::Div => Literal::Int(x.wrapping_div(y)),
            BinOp::Rem => Literal::Int(x.wrapping_rem(y)),
            BinOp::BitAnd => Literal::Int(x & y),
            BinOp::BitOr => Literal::Int(x | y),
            BinOp::BitXor => Literal::Int(x ^ y),
            BinOp::Shl => Literal::Int(x.wrapping_shl(shift(y)?)),
            BinOp::Shr => Literal::Int(x.wrapping_shr(shift(y)?)),
            BinOp::Eq => Literal::Bool(x == y),
            BinOp::Ne => Literal::Bool(x != y),
            BinOp::Lt => Literal::Bool(x < y),
            BinOp::Le => Literal::Bool(x <= y),
            BinOp::Gt => Literal::Bool(x > y),
            BinOp::Ge => Literal::Bool(x >= y),
        }));
    }
    // Compare known literals directly (avoiding O(len_a + len_b) BLAKE3 hashing
    // on mismatched Bytes/Str), and fall back to canonical cairn equality for
    // shades and answers. `spec/05-types.md` §5.3.
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        if let (Kind::Known(la), Kind::Known(lb)) = (&a.kind, &b.kind) {
            return Ok(Some(Literal::Bool(matches!(op, BinOp::Eq) == (la == lb))));
        }
        if let (Some(va), Some(vb)) = (as_value(a), as_value(b)) {
            return Ok(Some(Literal::Bool(matches!(op, BinOp::Eq) == (va.cairn() == vb.cairn()))));
        }
    }
    Ok(None)
}

/// The ledger value a burial value is, when it is one.
pub(crate) fn as_value(v: &Val) -> Option<Value> {
    match &v.kind {
        Kind::Known(ValLit::Unit) => Some(Value::Unit),
        Kind::Known(ValLit::Bool(b)) => Some(Value::Bool(*b)),
        Kind::Known(ValLit::Int(n)) => Some(Value::Int(*n)),
        Kind::Known(ValLit::Bytes(b)) => Some(Value::Bytes(b.to_vec())),
        Kind::Known(ValLit::Str(s)) => Some(Value::Str(s.to_string())),
        Kind::Known(ValLit::Cairn(c)) => Some(Value::Cairn(Cairn::from_bytes(*c))),
        // Only the origin and the name survive, which is what a shade is.
        Kind::Shade(inner) => {
            let Type::Shade { origin, .. } = &v.ty else { return None };
            as_value(inner).map(|held| Value::Shade {
                origin: Stratum::new(origin.get()).unwrap_or(Stratum::MAX),
                value: held.cairn(),
            })
        }
        // §5.1.1 gives a refusal and an answer each a tag, so both are values
        // the ledger can hold.
        Kind::Known(ValLit::Refusal(r)) => Some(Value::Refusal(ledger_code(*r))),
        Kind::Refused(r) => {
            Some(Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(ledger_code(*r)))))
        }
        Kind::Answered(inner) => as_value(inner)
            .map(|held| Value::Answer(Box::new(nether_ledger::AnswerOf::Given(held)))),
        // A function and a prelude function have no written form at all.
        Kind::Func(_) | Kind::Prim(_) | Kind::Stuck(_) => None,
    }
}

/// §5.1.1's six, from the ledger's spelling to the calculus's.
pub(crate) fn code(r: nether_ledger::Refusal) -> RefusalCode {
    match r {
        nether_ledger::Refusal::Absent => RefusalCode::Absent,
        nether_ledger::Refusal::Denied => RefusalCode::Denied,
        nether_ledger::Refusal::Malformed => RefusalCode::Malformed,
        nether_ledger::Refusal::Unreachable => RefusalCode::Unreachable,
        nether_ledger::Refusal::Exhausted => RefusalCode::Exhausted,
        nether_ledger::Refusal::Conflict => RefusalCode::Conflict,
    }
}

/// And back the other way.
fn ledger_code(r: RefusalCode) -> nether_ledger::Refusal {
    match r {
        RefusalCode::Absent => nether_ledger::Refusal::Absent,
        RefusalCode::Denied => nether_ledger::Refusal::Denied,
        RefusalCode::Malformed => nether_ledger::Refusal::Malformed,
        RefusalCode::Unreachable => nether_ledger::Refusal::Unreachable,
        RefusalCode::Exhausted => nether_ledger::Refusal::Exhausted,
        RefusalCode::Conflict => nether_ledger::Refusal::Conflict,
    }
}

/// A value the world said, as a literal, when it has a form.
pub(crate) fn literal(v: &Value) -> Option<Literal> {
    Some(match v {
        Value::Unit => Literal::Unit,
        Value::Bool(b) => Literal::Bool(*b),
        Value::Int(n) => Literal::Int(*n),
        Value::Bytes(b) => Literal::Bytes(b.clone()),
        Value::Str(s) => Literal::Str(s.clone()),
        Value::Cairn(c) => Literal::Cairn(*c.as_bytes()),
        Value::Refusal(r) => Literal::Refusal(code(*r)),
        // §04 has no syntax for an aggregate, a shade or an answer, so none of
        // them can be a literal. 0116.
        Value::Struct { .. } | Value::Array(_) | Value::Shade { .. } | Value::Answer(_) => {
            return None;
        }
    })
}

/// A shift by something no shift can mean (`< 0` or `>= 64`).
fn shift(n: i64) -> Result<u32, &'static str> {
    u32::try_from(n).ok().filter(|&s| s < 64).ok_or("this shifts by a width that is not one")
}

/// Byte work performed by a pure prelude operation for fuel accounting.
pub(crate) fn prim_byte_work(p: Prim, args: &[Val]) -> usize {
    let bytes = |i: usize| args.get(i).and_then(Val::bytes);
    match p {
        Prim::Concat => bytes(0).zip(bytes(1)).map_or(0, |(a, b)| a.len().saturating_add(b.len())),
        Prim::Slice => {
            match (bytes(0), args.get(1).and_then(Val::int), args.get(2).and_then(Val::int)) {
                (Some(b), Some(from), Some(to)) => usize::try_from(from)
                    .ok()
                    .zip(usize::try_from(to).ok())
                    .and_then(|(f, t)| b.get(f..t))
                    .map_or(0, <[u8]>::len),
                _ => 0,
            }
        }
        Prim::StartsWith => bytes(1).map_or(0, <[u8]>::len),
        Prim::Raw | Prim::CairnOf => bytes(0).map_or(0, <[u8]>::len),
        _ => 0,
    }
}

pub(crate) enum PrimErr {
    Collapsed(&'static str),
    TooLarge(usize),
}

/// The prelude functions burial can finish.
///
/// Every one of them is at depth 0 and produces something with a written form.
/// `spec/09-prelude.md` §9.2.
pub(crate) fn prim(p: Prim, args: &[Val]) -> Result<Option<Literal>, PrimErr> {
    let int = |i: usize| args.get(i).and_then(Val::int);
    let bytes = |i: usize| args.get(i).and_then(Val::bytes);
    let two = |f: fn(i64, i64) -> i64| int(0).zip(int(1)).map(|(a, b)| Literal::Int(f(a, b)));
    Ok(match p {
        Prim::Min => two(i64::min),
        Prim::Max => two(i64::max),
        Prim::Abs => int(0).map(|n| Literal::Int(n.wrapping_abs())),
        Prim::Len => match bytes(0).map(|b| i64::try_from(b.len())) {
            Some(Ok(n)) => Some(Literal::Int(n)),
            Some(Err(_)) => return Err(PrimErr::Collapsed("this is longer than an I64 can count")),
            None => None,
        },
        Prim::Concat => match bytes(0).zip(bytes(1)) {
            Some((a, b)) => {
                let total = a
                    .len()
                    .checked_add(b.len())
                    .filter(|&n| n <= MAX_CONCAT)
                    .ok_or(PrimErr::TooLarge(MAX_CONCAT))?;
                let mut out = Vec::with_capacity(total);
                out.extend_from_slice(a);
                out.extend_from_slice(b);
                Some(Literal::Bytes(out))
            }
            None => None,
        },
        Prim::Slice => match (bytes(0), int(1), int(2)) {
            (Some(b), Some(from), Some(to)) => {
                let range = usize::try_from(from).ok().zip(usize::try_from(to).ok());
                // An out-of-range slice cannot produce a value and never will.
                // `spec/09-prelude.md` §9.9.
                match range.and_then(|(f, t)| b.get(f..t)) {
                    Some(cut) => Some(Literal::Bytes(cut.to_vec())),
                    None => {
                        return Err(PrimErr::Collapsed("this slice is outside what it is slicing"));
                    }
                }
            }
            _ => None,
        },
        Prim::StartsWith => bytes(0).zip(bytes(1)).map(|(b, p)| Literal::Bool(b.starts_with(p))),
        Prim::Raw => bytes(0).map(|b| Literal::Bytes(b.to_vec())),
        Prim::CairnOf => {
            bytes(0).map(|b| Literal::Cairn(*Value::Bytes(b.to_vec()).cairn().as_bytes()))
        }
        Prim::Hex => match args.first().map(|v| &v.kind) {
            Some(Kind::Known(ValLit::Cairn(c))) => {
                Some(Literal::Str(Cairn::from_bytes(*c).to_string()))
            }
            _ => None,
        },
        _ => None,
    })
}
