//! The IR, written back out as source.
//!
//! Here so that lowering can be proved rather than inspected: source that
//! lowers to an IR that prints to source that lowers to the same IR is source
//! nothing was lost from. The folds are undone on the way out — a branch
//! becomes an `if` again, a loop becomes a `for`, and a select whose arms are
//! a constant becomes the `&&` or `||` it came from.
//!
//! It is not a formatter, and it parenthesises exactly where §4.6 needs it to:
//! a child whose level is looser than its place allows, read from the same
//! table the parser climbs. That is what keeps a printed residue inside
//! `MAX_NESTING` — minimal parentheses never nest deeper than the source that
//! lowered — and §6.5 needs every residue to be read back in. 0279.

use core::fmt::Write as _;

use nether_core::{
    Asserted, BinOp, Block, Expr, ExprKind, FuncDef, Literal, LocalDef, LocalId, Proj, Rite, Stmt,
    Type, UnOp, Unit,
};

use crate::parse::LEVELS;

/// Write a unit out as Nether C.
#[must_use]
pub fn print(unit: &Unit) -> String {
    let mut out = String::new();
    for s in &unit.structs {
        let _ = writeln!(out, "struct {} {{", s.name);
        for f in &s.fields {
            let _ = writeln!(out, "  {} {};", declared(&f.ty, f.asserted), f.name);
        }
        let _ = writeln!(out, "}};\n");
    }
    for g in &unit.globals {
        let _ = writeln!(
            out,
            "{} {} = {};",
            declared(&g.ty, g.asserted),
            g.name,
            expr(&g.value, unit, None)
        );
    }
    if !unit.globals.is_empty() {
        out.push('\n');
    }
    for f in &unit.funcs {
        let params: Vec<String> = f
            .params
            .iter()
            .filter_map(|p| f.local(*p))
            .map(|p| format!("{} {}", declared(&p.ty, p.asserted), p.name))
            .collect();
        // Only what was written is written back. A latent depth the source did
        // not assert is not one the round trip may invent.
        let latent = f.asserted_latent.map_or_else(String::new, |d| format!(" @{d}"));
        let _ = writeln!(
            out,
            "{} {}({}){latent}",
            declared(&f.ret, f.asserted_ret),
            f.name,
            params.join(", ")
        );
        let _ = writeln!(out, "{}\n", block(&f.body, unit, Some(f), 0));
    }
    for d in &unit.demands {
        let _ = writeln!(out, "demand {};", expr(&d.value, unit, None));
    }
    out
}

/// A type, as §4.3 writes one. A shade never writes its origin: §4.3 is
/// explicit that writing it would let a program claim one it does not have.
fn ty(t: &Type) -> String {
    match t {
        Type::Unit => "U0".into(),
        Type::Bool => "Bool".into(),
        Type::Int => "I64".into(),
        Type::Bytes => "Bytes".into(),
        Type::Str => "Str".into(),
        Type::Cairn => "Cairn".into(),
        Type::Refusal => "Refusal".into(),
        Type::Shade { inner, .. } => format!("Shade<{}>", ty(inner)),
        Type::Answer(inner) => format!("Answer<{}>", ty(inner)),
        Type::Struct(name) => name.clone(),
        Type::Array { elem, len } => match len {
            Some(n) => format!("{}[{n}]", ty(elem)),
            None => format!("{}[]", ty(elem)),
        },
        // §04 has no syntax for one, so a unit that came from source has none
        // in a position this reaches. Printed so that the round trip says so
        // loudly rather than quietly producing something else.
        Type::Fn { .. } => "«a function type has no syntax»".into(),
    }
}

/// A type as a declaration writes it, with the depth that was asserted on it.
///
/// §5.6: an annotation is a checked assertion and is optional, so printing one
/// the source did not write would be adding a claim to the program.
fn declared(t: &Type, asserted: Asserted) -> String {
    match asserted {
        Some(d) => format!("{}@{d}", ty(t)),
        None => ty(t),
    }
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

/// A local, from the function it is in or, outside one, from the unit. 0280.
fn local<'u>(unit: &'u Unit, f: Option<&'u FuncDef>, id: LocalId) -> Option<&'u LocalDef> {
    f.map_or_else(|| unit.local(id), |f| f.local(id))
}

fn local_name(unit: &Unit, f: Option<&FuncDef>, id: LocalId) -> String {
    local(unit, f, id).map_or_else(|| format!("«local {}»", id.0), |l| l.name.clone())
}

fn block(b: &Block, unit: &Unit, f: Option<&FuncDef>, at: usize) -> String {
    let mut out = format!("{}{{\n", indent(at));
    for s in &b.stmts {
        out.push_str(&stmt(s, unit, f, at + 1));
    }
    if let Some(tail) = &b.tail {
        let _ = writeln!(out, "{}{}", indent(at + 1), expr(tail, unit, f));
    }
    let _ = write!(out, "{}}}", indent(at));
    out
}

fn stmt(s: &Stmt, unit: &Unit, f: Option<&FuncDef>, at: usize) -> String {
    match s {
        Stmt::Let { local, value } => {
            let def = self::local(unit, f, *local);
            let (t, name) = def.map_or_else(
                || ("U0".to_string(), format!("«local {}»", local.0)),
                |d| (declared(&d.ty, d.asserted), d.name.clone()),
            );
            format!("{}{t} {name} = {};\n", indent(at), expr(value, unit, f))
        }
        // A declaration with no value, which is how an aggregate is built. It
        // has to print back as one or a residue stops round-tripping. §6.5.
        Stmt::Declare { local } => {
            let def = self::local(unit, f, *local);
            let (t, name) = def.map_or_else(
                || ("U0".to_string(), format!("«local {}»", local.0)),
                |d| (declared(&d.ty, d.asserted), d.name.clone()),
            );
            format!("{}{t} {name};\n", indent(at))
        }
        // The forms that were statements before lowering folded them go back
        // to being statements, which is what makes the result parse.
        Stmt::Expr(x) => match &x.kind {
            ExprKind::Select { .. } if !short_circuit(x) => {
                format!("{}\n", branch(x, unit, f, at))
            }
            ExprKind::Loop { body, step } => {
                let step =
                    step.as_ref().map_or_else(String::new, |s| format!(" {}", expr(s, unit, f)));
                let body = as_block(body, unit, f, at);
                format!("{}for (; ;{step})\n{body}\n", indent(at))
            }
            ExprKind::Break => format!("{}break;\n", indent(at)),
            ExprKind::Continue => format!("{}continue;\n", indent(at)),
            ExprKind::Return(v) => match v {
                Some(v) => format!("{}return {};\n", indent(at), expr(v, unit, f)),
                None => format!("{}return;\n", indent(at)),
            },
            ExprKind::Block(b) => format!("{}\n", block(b, unit, f, at)),
            _ => format!("{}{};\n", indent(at), expr(x, unit, f)),
        },
    }
}

/// A select that came from an `if`, written back as one. A missing `else` is
/// the `U0` lowering put there, and goes away again.
fn branch(x: &Expr, unit: &Unit, f: Option<&FuncDef>, at: usize) -> String {
    let ExprKind::Select { cond, then, otherwise } = &x.kind else { return String::new() };
    let mut out =
        format!("{}if ({})\n{}", indent(at), expr(cond, unit, f), as_block(then, unit, f, at));
    // An `else if` chain is a select in the otherwise arm, and goes back to
    // being a chain rather than a block holding a branch.
    if matches!(otherwise.kind, ExprKind::Select { .. }) && !short_circuit(otherwise) {
        let nested = branch(otherwise, unit, f, at);
        let _ = write!(out, "\n{}else {}", indent(at), nested.trim_start());
    } else if !matches!(otherwise.kind, ExprKind::Literal(Literal::Unit)) {
        let _ = write!(out, "\n{}else\n{}", indent(at), as_block(otherwise, unit, f, at));
    }
    out
}

/// An expression in a position that wants a block.
fn as_block(x: &Expr, unit: &Unit, f: Option<&FuncDef>, at: usize) -> String {
    match &x.kind {
        ExprKind::Block(b) => block(b, unit, f, at),
        ExprKind::Break => format!("{}{{\n{}break;\n{}}}", indent(at), indent(at + 1), indent(at)),
        ExprKind::Continue => {
            format!("{}{{\n{}continue;\n{}}}", indent(at), indent(at + 1), indent(at))
        }
        ExprKind::Literal(Literal::Unit) => format!("{}{{\n{}}}", indent(at), indent(at)),
        _ => format!("{}{{\n{}{}\n{}}}", indent(at), indent(at + 1), expr(x, unit, f), indent(at)),
    }
}

/// Whether this select is one of the two that were operators.
fn short_circuit(x: &Expr) -> bool {
    let ExprKind::Select { then, otherwise, .. } = &x.kind else { return false };
    matches!(otherwise.kind, ExprKind::Literal(Literal::Bool(false)))
        || matches!(then.kind, ExprKind::Literal(Literal::Bool(true)))
}

/// An expression in a place that takes any expression.
fn expr(x: &Expr, unit: &Unit, f: Option<&FuncDef>) -> String {
    within(x, ASSIGNMENT, unit, f)
}

/// §4.6's levels: postfix 1, prefix 2, the binary ladder 3 to 12, assignment
/// 13. Everything that is not an operator is 0.
const POSTFIX: u8 = 1;
const PREFIX: u8 = 2;
const ASSIGNMENT: u8 = 13;

/// The level an operator sits at, by the spelling the parser reads.
fn level(spelled: &str) -> u8 {
    LEVELS.iter().find(|(p, _, _)| p.spelling() == spelled).map_or(ASSIGNMENT, |(_, l, _)| *l)
}

/// The level of what this expression prints as.
fn level_of(x: &Expr) -> u8 {
    match &x.kind {
        ExprKind::Literal(Literal::Int(n)) if *n < 0 => PREFIX,
        ExprKind::Call { .. } | ExprKind::Index { .. } | ExprKind::Field { .. } => POSTFIX,
        ExprKind::Unary { .. } | ExprKind::Rite { .. } => PREFIX,
        ExprKind::Binary { op, .. } => level(binary(*op)),
        // The order `bare` tests them in, so the two never disagree.
        ExprKind::Select { otherwise, .. } => level(if is_false(otherwise) { "&&" } else { "||" }),
        ExprKind::Assign { .. } => ASSIGNMENT,
        _ => 0,
    }
}

/// An expression in a place that takes nothing looser than `allowed`.
fn within(x: &Expr, allowed: u8, unit: &Unit, f: Option<&FuncDef>) -> String {
    let bare = bare(x, unit, f);
    if level_of(x) > allowed { format!("({bare})") } else { bare }
}

fn bare(x: &Expr, unit: &Unit, f: Option<&FuncDef>) -> String {
    let go = |e: &Expr| expr(e, unit, f);
    let at = |e: &Expr, allowed: u8| within(e, allowed, unit, f);
    match &x.kind {
        ExprKind::Literal(l) => literal(l),
        ExprKind::Local(id) => local_name(unit, f, *id),
        ExprKind::Global(id) => {
            unit.global(*id).map_or_else(|| format!("«global {}»", id.0), |g| g.name.clone())
        }
        ExprKind::Func(id) => {
            unit.func(*id).map_or_else(|| format!("«func {}»", id.0), |g| g.name.clone())
        }
        ExprKind::Prim(p) => p.name().to_string(),
        ExprKind::Call { callee, args } => {
            let args: Vec<String> = args.iter().map(go).collect();
            format!("{}({})", at(callee, POSTFIX), args.join(", "))
        }
        ExprKind::Unary { op, operand } => format!("{}{}", unary(*op), at(operand, PREFIX)),
        // Left-associative, every level of it: the left may sit at this
        // level and the right may not.
        ExprKind::Binary { op, lhs, rhs } => {
            let l = level(binary(*op));
            format!("{} {} {}", at(lhs, l), binary(*op), at(rhs, l - 1))
        }
        ExprKind::Field { base, index } => {
            let name = field_name(&base.ty, *index, unit);
            format!("{}.{name}", at(base, POSTFIX))
        }
        ExprKind::Index { base, index } => format!("{}[{}]", at(base, POSTFIX), go(index)),
        ExprKind::Assign { place, value } => {
            let mut out = local_name(unit, f, place.local);
            let mut ty_of = local(unit, f, place.local).map(|l| l.ty.clone());
            for p in &place.path {
                match p {
                    Proj::Field(i) => {
                        let name = ty_of
                            .as_ref()
                            .map_or_else(|| format!("«{i}»"), |t| field_name(t, *i, unit));
                        ty_of = ty_of.as_ref().and_then(|t| field_type(t, *i, unit));
                        let _ = write!(out, ".{name}");
                    }
                    Proj::Index(e) => {
                        ty_of = ty_of.and_then(|t| match t {
                            Type::Array { elem, .. } => Some(*elem),
                            _ => None,
                        });
                        let _ = write!(out, "[{}]", go(e));
                    }
                }
            }
            format!("{out} = {}", go(value))
        }
        // A block in expression position is its own tail, which is the only
        // way §04 lets one appear there.
        ExprKind::Block(b) => block(b, unit, f, 0),
        ExprKind::Select { cond, then, otherwise } => {
            if is_false(otherwise) {
                format!("{} && {}", at(cond, level("&&")), at(then, level("&&") - 1))
            } else if matches!(then.kind, ExprKind::Literal(Literal::Bool(true))) {
                format!("{} || {}", at(cond, level("||")), at(otherwise, level("||") - 1))
            } else {
                // §04 has no `if` expression, so this cannot have come from
                // source and cannot go back to it.
                "«a branch outside a statement»".into()
            }
        }
        ExprKind::Loop { .. } => "«a loop outside a statement»".into(),
        ExprKind::Break => "break".into(),
        ExprKind::Continue => "continue".into(),
        ExprKind::Return(_) => "«a return outside a statement»".into(),
        ExprKind::Descend { capability, body } => {
            format!("descend {} {}", capability.name(), as_block(body, unit, f, 0))
        }
        ExprKind::Rite { rite, operand } => {
            let word = match rite {
                Rite::Seal => "seal",
                Rite::Shade => "shade",
                Rite::Look => "look",
                Rite::Opaque => "opaque",
            };
            format!("{word} {}", at(operand, PREFIX))
        }
    }
}

fn is_false(x: &Expr) -> bool {
    matches!(x.kind, ExprKind::Literal(Literal::Bool(false)))
}

fn field_name(t: &Type, index: u32, unit: &Unit) -> String {
    let Type::Struct(name) = t else { return format!("«{index}»") };
    unit.struct_def(name)
        .and_then(|s| s.fields.get(index as usize))
        .map_or_else(|| format!("«{index}»"), |f| f.name.clone())
}

fn field_type(t: &Type, index: u32, unit: &Unit) -> Option<Type> {
    let Type::Struct(name) = t else { return None };
    unit.struct_def(name).and_then(|s| s.fields.get(index as usize)).map(|f| f.ty.clone())
}

fn literal(l: &Literal) -> String {
    match l {
        // §04 has no literal for `U0`, and an empty block is the value it
        // would have named.
        Literal::Unit => "{ }".into(),
        Literal::Bool(b) => b.to_string(),
        Literal::Int(n) => n.to_string(),
        Literal::Str(s) => spelled(s),
        Literal::Bytes(b) => quoted(b),
        Literal::Refusal(r) => r.name().to_string(),
        // §3.6: `#` and sixty-four lowercase hex digits, which is what the
        // rites print. §6.5 needs this: a residue is a program, and a burial
        // that folds `seal` has a cairn to write down.
        Literal::Cairn(c) => {
            let mut out = String::with_capacity(65);
            out.push('#');
            for byte in c {
                let _ = write!(out, "{byte:02x}");
            }
            out
        }
    }
}

/// A bytes literal, escaped the way §3.6 escapes one.
///
/// A byte outside printable ASCII is `\xNN` and not `\u{…}`: the escape that
/// names a scalar value would be encoded on the way back in, and two bytes
/// written as one character come back as four. §6.5 requires that lowering
/// what was printed gives back the same program.
fn quoted(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for b in bytes {
        match b {
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0 => out.push_str("\\0"),
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            0x20..=0x7e => out.push(char::from(*b)),
            other => {
                let _ = write!(out, "\\x{other:02x}");
            }
        }
    }
    out.push('"');
    out
}

/// A string literal, escaped the way §3.6 escapes one.
///
/// By character rather than by byte, because a `Str` is UTF-8 and the source
/// it is printed into is too (§3.1). A character that is already text is
/// written as itself; a control character has no printable form and is
/// `\u{…}`, which is what that escape is for.
fn spelled(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn unary(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "-",
        UnOp::Not => "!",
        UnOp::BitNot => "~",
    }
}

fn binary(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
    }
}
