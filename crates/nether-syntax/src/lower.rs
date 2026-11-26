//! AST to IR: name resolution, desugaring, and where `@n` stops being syntax.
//!
//! Three things happen here and nowhere else.
//!
//! **Names become positions.** An identifier is a local, a unit-level binding,
//! a function, a prelude function or a refusal constant, and after this nobody
//! asks again.
//!
//! **Spelling is folded away.** `&&` and `||` and `if` become one branch,
//! `while` and `for` become one loop, `x += e` becomes an assignment of a sum.
//! The folds are listed in `nether_core::ir`, and the reason they happen here
//! rather than in the parser is that a parser that folds reports errors about
//! a program nobody wrote.
//!
//! **Depths are derived.** Every IR node states one, and this is where it
//! comes from — by the rules in `spec/02-calculus.md` §2.2, implemented a
//! second time. The checker implements them too, and `nether_core::check` on
//! the result is two implementations agreeing rather than one agreeing with
//! itself.

use std::collections::{HashMap, HashSet};

// Lowering is recursive and has no bound of its own: it walks what `parse`
// produced, and `parse` will not produce anything deeper than `MAX_NESTING`.
use nether_core::{self as ir, Asserted, Capability, Depth, Held, Prim, Refusal, Rite, Span, UnOp};

use crate::ast;
use crate::lex::{Fault, FaultKind};

type FuncSig = (Vec<(ir::Type, Asserted)>, (ir::Type, Asserted));

/// Upper bound on loop fixpoint re-iterations per unit.
const MAX_FIXPOINT_STEPS: usize = 4096;

/// Upper bound on expanded type nesting depth (matching `parse::MAX_NESTING`).
const MAX_TYPE_DEPTH: usize = 64;

/// Control-flow exit depths propagated out of a statement or expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExitPc {
    loop_exit: Option<Depth>,
    fn_exit: Option<Depth>,
}

impl ExitPc {
    const NONE: Self = Self { loop_exit: None, fn_exit: None };

    fn join(self, other: Self) -> Self {
        Self {
            loop_exit: join_opt(self.loop_exit, other.loop_exit),
            fn_exit: join_opt(self.fn_exit, other.fn_exit),
        }
    }

    fn deepest(self) -> Option<Depth> {
        join_opt(self.loop_exit, self.fn_exit)
    }
}

const fn is_poly_prim(p: Prim) -> bool {
    matches!(p, Prim::Refused | Prim::Answered | Prim::Must | Prim::Given | Prim::Refusal)
}

fn join_opt(a: Option<Depth>, b: Option<Depth>) -> Option<Depth> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.join(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

/// Lower a parsed unit.
///
/// # Errors
///
/// Every name it could not resolve and every shape it could not make sense
/// of, in source order.
pub fn lower(unit: &ast::Unit) -> Result<ir::Unit, Vec<Fault>> {
    let mut l = Lowering::new(unit);
    let out = l.unit(unit);
    if l.faults.is_empty() { Ok(out) } else { Err(l.faults) }
}

#[derive(Clone, Copy)]
enum TypeDecl<'a> {
    Alias(&'a ast::Type),
    Struct(&'a [ast::Field]),
}

struct Lowering<'a> {
    faults: Vec<Fault>,
    /// `typedef` and `struct` declarations, mutually exclusive by name.
    types: HashMap<&'a str, TypeDecl<'a>>,
    /// Active typedef names during expansion, for cycle detection.
    expanding: Vec<&'a str>,
    /// Memoized clean typedef expansions `(type, asserted, has_inferred, height)`.
    expanded_aliases: HashMap<&'a str, (ir::Type, Asserted, bool, usize)>,
    /// Unit-level functions and bindings, by name.
    funcs: HashMap<&'a str, (ir::FuncId, ir::Type)>,
    globals: HashMap<&'a str, (ir::GlobalId, ir::Type, Depth)>,
    /// The locals of the function being lowered, innermost scope last.
    scopes: Vec<Vec<(String, ir::LocalId)>>,
    locals: Vec<ir::LocalDef>,
    /// Names already used in the current local frame (for O(1) temp naming).
    local_names: HashSet<String>,
    /// Next candidate index for `_idx{n}` temporary locals.
    next_temp: usize,
    /// The depth of every local bound so far, by position.
    depths: Vec<Depth>,
    /// The declared type of what is being lowered, while it is the whole of a
    /// binding's value or a `return`'s. `refused` has no other way to know its
    /// `T`. §9.2.
    landing: Option<ir::Type>,
    /// What the function being lowered returns.
    returning: Option<ir::Type>,
    /// Deepest value any `return` in the current function carries out (including `pc`).
    ret_depth: Depth,
    /// Current control-dependence depth (from enclosing branch/loop conditions).
    pc: Depth,
    /// Exits observed in the expression or statement just lowered.
    last_exit: ExitPc,
    /// Nesting depth of enclosing loops (for validating `break`/`continue`).
    in_loop: u32,
    /// Total loop fixpoint re-iterations used in this unit.
    fixpoint_steps: usize,
    /// Unit-level code's locals, names, temp counter, and depths, kept while a
    /// function's are in their place.
    unit_frame: (Vec<ir::LocalDef>, HashSet<String>, usize, Vec<Depth>),
}

impl<'a> Lowering<'a> {
    fn new(unit: &'a ast::Unit) -> Self {
        let mut l = Self {
            faults: Vec::new(),
            types: HashMap::new(),
            expanding: Vec::new(),
            expanded_aliases: HashMap::new(),
            funcs: HashMap::new(),
            globals: HashMap::new(),
            scopes: Vec::new(),
            locals: Vec::new(),
            local_names: HashSet::new(),
            next_temp: 0,
            depths: Vec::new(),
            landing: None,
            returning: None,
            ret_depth: Depth::PURE,
            pc: Depth::PURE,
            last_exit: ExitPc::NONE,
            in_loop: 0,
            fixpoint_steps: 0,
            unit_frame: (Vec::new(), HashSet::new(), 0, Vec::new()),
        };
        // Declarations are visible to each other regardless of order, so the
        // shapes are collected before anything is lowered.
        for item in &unit.items {
            let (name, decl) = match item {
                ast::Item::Typedef { ty, name, .. } => (name, TypeDecl::Alias(ty)),
                ast::Item::Struct { name, fields, .. } => {
                    (name, TypeDecl::Struct(fields.as_slice()))
                }
                _ => continue,
            };
            if l.types.contains_key(name.text.as_str()) {
                l.fault(name.span, FaultKind::Expected("a unique type name"));
            } else {
                l.types.insert(name.text.as_str(), decl);
            }
        }
        l
    }

    fn fault(&mut self, span: Span, kind: FaultKind) {
        self.faults.push(Fault { span, kind });
    }

    // ── types ───────────────────────────────────────────────────────────────

    /// An `ast::Type`, with aliases expanded and prelude names resolved.
    fn ty(&mut self, t: &ast::Type) -> ir::Type {
        self.ty_in(t, false).0
    }

    /// Lower an `ast::Type`, returning `(ir_type, effective_asserted_depth, has_inferred_shade)`.
    /// When `allow_inferred_shade` is true (initialized `let` bindings), direct
    /// `Shade<T>` annotations may omit `@d` and inherit the RHS origin.
    fn ty_in(&mut self, t: &ast::Type, allow_inferred_shade: bool) -> (ir::Type, Asserted, bool) {
        let (ty, asserted, has_inferred, _) = self.ty_depth(t, allow_inferred_shade);
        (ty, asserted, has_inferred)
    }

    fn ty_depth(
        &mut self,
        t: &ast::Type,
        allow_inferred_shade: bool,
    ) -> (ir::Type, Asserted, bool, usize) {
        let (mut inner, alias_asserted, has_inferred, mut height) =
            self.atom_in(t, allow_inferred_shade);
        for len in t.arrays.iter().rev() {
            height = height.saturating_add(1);
            if height > MAX_TYPE_DEPTH {
                self.fault(t.span, FaultKind::TooDeep);
                return (ir::Type::Unit, None, false, MAX_TYPE_DEPTH);
            }
            let len = match len {
                Some(n) if *n < 0 => {
                    self.fault(t.span, FaultKind::Expected("a non-negative array length"));
                    None
                }
                Some(n) => u64::try_from(*n).ok(),
                None => None,
            };
            inner = ir::Type::Array { elem: Box::new(inner), len };
        }
        let own = Self::asserted(t);
        if let (Some(a), Some(b)) = (own, alias_asserted)
            && a != b
        {
            self.fault(t.span, FaultKind::Expected("a matching depth annotation on typedef"));
        }
        (inner, own.or(alias_asserted), has_inferred, height)
    }

    fn expand_alias(
        &mut self,
        key: &'a str,
        alias: &'a ast::Type,
        span: Span,
    ) -> (ir::Type, Asserted, bool, usize) {
        if let Some(cached) = self.expanded_aliases.get(key) {
            return cached.clone();
        }
        if self.expanding.contains(&key) || self.expanding.len() > 16 {
            self.fault(span, FaultKind::Expected("a non-cyclic typedef"));
            return (ir::Type::Unit, None, false, 1);
        }
        let faults_before = self.faults.len();
        self.expanding.push(key);
        let expanded = self.ty_depth(alias, false);
        self.expanding.pop();
        if self.faults.len() == faults_before {
            self.expanded_aliases.insert(key, expanded.clone());
        }
        expanded
    }

    fn atom_in(
        &mut self,
        t: &ast::Type,
        allow_inferred_shade: bool,
    ) -> (ir::Type, Asserted, bool, usize) {
        let name = t.name.text.as_str();
        let reject_args = |this: &mut Self, ok_len: usize| {
            if t.args.len() > ok_len {
                this.fault(t.span, FaultKind::Expected("no extra type arguments"));
            }
        };
        // §3.4: user-defined typedefs and structs shadow prelude type names.
        if let Some((&key, &decl)) = self.types.get_key_value(name) {
            reject_args(self, 0);
            return match decl {
                TypeDecl::Alias(alias) => self.expand_alias(key, alias, t.span),
                TypeDecl::Struct(_) => (ir::Type::Struct(name.to_string()), None, false, 1),
            };
        }
        let mut has_inferred = false;
        let mut height = 1usize;
        let ty = match name {
            "U0" | "Bool" | "I64" | "Bytes" | "Str" | "Cairn" | "Refusal" => {
                reject_args(self, 0);
                match name {
                    "U0" => ir::Type::Unit,
                    "Bool" => ir::Type::Bool,
                    "I64" => ir::Type::Int,
                    "Bytes" => ir::Type::Bytes,
                    "Str" => ir::Type::Str,
                    "Cairn" => ir::Type::Cairn,
                    _ => ir::Type::Refusal,
                }
            }
            "Answer" => {
                reject_args(self, 1);
                if let Some(arg_ast) = t.args.first() {
                    let (a, arg_asserted, arg_inf, arg_h) =
                        self.ty_depth(arg_ast, allow_inferred_shade);
                    has_inferred = arg_inf;
                    height = arg_h.saturating_add(1);
                    if height > MAX_TYPE_DEPTH {
                        self.fault(t.span, FaultKind::TooDeep);
                        return (ir::Type::Unit, None, false, MAX_TYPE_DEPTH);
                    }
                    if arg_asserted.is_some() {
                        self.fault(
                            arg_ast.span,
                            FaultKind::Expected(
                                "no depth annotation inside `Answer<T>` (write `Answer<T>@d` instead)",
                            ),
                        );
                    }
                    ir::Type::Answer(Box::new(a))
                } else {
                    self.fault(t.span, FaultKind::Expected("`Answer<T>` to say what it answers"));
                    ir::Type::Unit
                }
            }
            // §4.3: a received shade MUST write its origin on its argument (`Shade<Bytes@5>`).
            // An initialized binding may omit it (`Shade<Bytes>`) and inherit it from the value.
            "Shade" => {
                reject_args(self, 1);
                if let Some(arg_ast) = t.args.first() {
                    let (arg_ir, arg_asserted, arg_inf, arg_h) =
                        self.ty_depth(arg_ast, allow_inferred_shade);
                    has_inferred = arg_inf;
                    height = arg_h.saturating_add(1);
                    if height > MAX_TYPE_DEPTH {
                        self.fault(t.span, FaultKind::TooDeep);
                        return (ir::Type::Unit, None, false, MAX_TYPE_DEPTH);
                    }
                    let origin = if let Some(d) = arg_asserted {
                        d
                    } else {
                        has_inferred = true;
                        if !allow_inferred_shade {
                            self.fault(
                                t.span,
                                FaultKind::Expected("an explicit `@d` origin on `Shade<T@d>`"),
                            );
                        }
                        Depth::UNRECORDED
                    };
                    ir::Type::Shade { origin, inner: Box::new(arg_ir) }
                } else {
                    self.fault(t.span, FaultKind::Expected("`Shade<T>` to say what it hides"));
                    ir::Type::Unit
                }
            }
            _ => {
                reject_args(self, 0);
                self.fault(t.span, FaultKind::Unknown("a type"));
                ir::Type::Unit
            }
        };
        (ty, None, has_inferred, height)
    }

    fn asserted(t: &ast::Type) -> Asserted {
        t.depth.and_then(Depth::new)
    }

    // ── the unit ────────────────────────────────────────────────────────────

    fn unit(&mut self, unit: &'a ast::Unit) -> ir::Unit {
        let mut structs = Vec::new();
        for item in &unit.items {
            if let ast::Item::Struct { name, fields, span } = item {
                let mut seen_fields: Vec<&str> = Vec::with_capacity(fields.len());
                let fields = fields
                    .iter()
                    .map(|f| {
                        let fname = f.name.text.as_str();
                        if seen_fields.contains(&fname) {
                            self.fault(f.name.span, FaultKind::Expected("a unique field name"));
                        } else {
                            seen_fields.push(fname);
                        }
                        let (ty, asserted, _) = self.ty_in(&f.ty, false);
                        ir::Field { name: f.name.text.clone(), ty, asserted, span: f.span }
                    })
                    .collect();
                structs.push(ir::StructDef { name: name.text.clone(), fields, span: *span });
            }
        }
        self.check_struct_cycles(&structs);

        // Validate every typedef declaration even if unreferenced.
        for item in &unit.items {
            if let ast::Item::Typedef { name, ty, .. } = item {
                self.expanding.clear();
                let key = name.text.as_str();
                self.expanding.push(key);
                let faults_before = self.faults.len();
                let expanded = self.ty_depth(ty, false);
                self.expanding.pop();
                if self.faults.len() == faults_before {
                    self.expanded_aliases.insert(key, expanded);
                }
            }
        }

        // Signatures before bodies, so a function can call one declared later.
        let declarations: Vec<&ast::Func> = unit
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Func(f) => Some(f),
                _ => None,
            })
            .collect();
        let mut sigs: Vec<FuncSig> = Vec::with_capacity(declarations.len());
        for (i, f) in declarations.iter().enumerate() {
            let param_sigs: Vec<(ir::Type, Asserted)> = f
                .params
                .iter()
                .map(|p| {
                    let (ty, asserted, _) = self.ty_in(&p.ty, false);
                    (ty, asserted)
                })
                .collect();
            let (ret, asserted_ret, _) = self.ty_in(&f.ret, false);
            let params = param_sigs.iter().map(|(ty, _)| ty.clone()).collect();
            // Start from lattice bottom (`Held::NONE`) so a recursive function's
            // written `@d` annotation cannot self-fulfill its own latent depth.
            let arrow = ir::Type::Fn {
                params,
                latent: Held::NONE,
                result: Box::new(ret.clone()),
                // What a function hands back is its body's depth, and the body
                // has not been lowered yet. Filled in below.
                result_depth: Depth::PURE,
            };
            sigs.push((param_sigs, (ret, asserted_ret)));
            let id = ir::FuncId(u32::try_from(i).unwrap_or(0));
            if self.funcs.insert(f.name.text.as_str(), (id, arrow)).is_some() {
                self.fault(f.name.span, FaultKind::Expected("a unique function name"));
            }
        }

        // A call lowered before its callee's body was is holding the wrong
        // result depth, so the whole thing is lowered again with the arrows
        // filled in, until nothing moves.
        let pre_loop_faults = self.faults.len();
        let mut globals = Vec::new();
        let mut funcs: Vec<ir::FuncDef> = Vec::new();
        for _ in 0..=declarations.len() {
            self.faults.truncate(pre_loop_faults);
            self.fixpoint_steps = 0;
            globals = self.unit_bindings(unit);
            funcs = declarations.iter().enumerate().map(|(i, f)| self.func(f, &sigs[i])).collect();
            let mut moved = false;
            for (i, f) in declarations.iter().enumerate() {
                let Some(entry) = self.funcs.get_mut(f.name.text.as_str()) else { continue };
                let ir::Type::Fn { result_depth, latent, .. } = &mut entry.1 else { continue };
                let derived = funcs[i].ret_depth;
                let needs = funcs[i].latent;
                moved |= *result_depth != derived || *latent != needs;
                *result_depth = derived;
                *latent = needs;
            }
            if !moved {
                break;
            }
        }
        // The bindings' frame, set aside while the functions had theirs, is
        // the demands' frame too: unit-level code has one.
        (self.locals, self.local_names, self.next_temp, self.depths) =
            std::mem::take(&mut self.unit_frame);
        let demands = self.demands(unit);
        let locals = std::mem::take(&mut self.locals);
        ir::Unit { structs, funcs, globals, demands, locals }
    }

    fn check_struct_cycles(&mut self, structs: &[ir::StructDef]) {
        fn collect_value_deps<'b>(ty: &'b ir::Type, out: &mut Vec<&'b str>) {
            match ty {
                ir::Type::Struct(name) => out.push(name.as_str()),
                ir::Type::Array { elem, len: Some(n) } if *n > 0 => collect_value_deps(elem, out),
                ir::Type::Shade { inner, .. } => collect_value_deps(inner, out),
                _ => {}
            }
        }
        fn dfs(idx: usize, adj: &[Vec<usize>], state: &mut [u8], cyclic: &mut [bool]) -> bool {
            state[idx] = 1;
            let mut found = false;
            for &next in &adj[idx] {
                if state[next] == 1 || (state[next] == 0 && dfs(next, adj, state, cyclic)) {
                    cyclic[idx] = true;
                    found = true;
                }
            }
            state[idx] = 2;
            found
        }
        let adj: Vec<Vec<usize>> = structs
            .iter()
            .map(|s| {
                let mut deps = Vec::new();
                for f in &s.fields {
                    collect_value_deps(&f.ty, &mut deps);
                }
                deps.into_iter()
                    .filter_map(|n| structs.iter().position(|st| st.name == n))
                    .collect()
            })
            .collect();
        let mut state = vec![0u8; structs.len()];
        let mut cyclic = vec![false; structs.len()];
        for i in 0..structs.len() {
            if state[i] == 0 {
                let _ = dfs(i, &adj, &mut state, &mut cyclic);
            }
        }
        for (i, is_cyc) in cyclic.into_iter().enumerate() {
            if is_cyc {
                self.fault(structs[i].span, FaultKind::Expected("a non-cyclic struct"));
            }
        }
    }

    fn unit_bindings(&mut self, unit: &'a ast::Unit) -> Vec<ir::GlobalDef> {
        self.globals.clear();
        self.locals = Vec::new();
        self.local_names.clear();
        self.next_temp = 0;
        self.depths = Vec::new();
        self.pc = Depth::PURE;
        self.in_loop = 0;
        let mut out = Vec::new();
        for item in &unit.items {
            let ast::Item::Let(binding) = item else { continue };
            self.scopes.push(Vec::new());
            // A global always has one; the parser requires it (§4.2).
            let Some(init) = binding.value.as_ref() else {
                self.scopes.pop();
                continue;
            };
            let (ty, asserted, has_inferred) = self.ty_in(&binding.ty, true);
            let value = self.landed(init, (!has_inferred).then(|| ty.clone()));
            self.scopes.pop();
            let ty = self.binding_type(&binding.ty, ty, &value);
            let id = ir::GlobalId(u32::try_from(out.len()).unwrap_or(0));
            if self.funcs.contains_key(binding.name.text.as_str())
                || self
                    .globals
                    .insert(binding.name.text.as_str(), (id, ty.clone(), value.depth))
                    .is_some()
            {
                self.fault(binding.name.span, FaultKind::Expected("a unique binding name"));
            }
            out.push(ir::GlobalDef {
                name: binding.name.text.clone(),
                ty,
                asserted,
                value,
                span: binding.span,
            });
        }
        self.unit_frame = (
            std::mem::take(&mut self.locals),
            std::mem::take(&mut self.local_names),
            self.next_temp,
            std::mem::take(&mut self.depths),
        );
        out
    }

    fn demands(&mut self, unit: &'a ast::Unit) -> Vec<ir::Demand> {
        self.pc = Depth::PURE;
        self.in_loop = 0;
        let mut out = Vec::new();
        for item in &unit.items {
            if let ast::Item::Demand { value, span } = item {
                self.scopes.push(Vec::new());
                let value = self.expr(value);
                self.scopes.pop();
                out.push(ir::Demand { value, span: *span });
            }
        }
        out
    }

    /// The declared type of a binding, recursively filling only omitted shade
    /// origins directly written on the binding (never overwriting `typedef` origins). §4.3.
    fn binding_type(&self, ty_ast: &ast::Type, ty: ir::Type, value: &ir::Expr) -> ir::Type {
        self.fill_shades(ty_ast, ty_ast.arrays.len(), ty, &value.ty)
    }

    fn fill_shades(
        &self,
        ty_ast: &ast::Type,
        arrays_left: usize,
        ir_ty: ir::Type,
        val_ty: &ir::Type,
    ) -> ir::Type {
        if arrays_left > 0 {
            return match (ir_ty, val_ty) {
                (ir::Type::Array { elem, len }, ir::Type::Array { elem: val_elem, .. }) => {
                    ir::Type::Array {
                        elem: Box::new(self.fill_shades(ty_ast, arrays_left - 1, *elem, val_elem)),
                        len,
                    }
                }
                (other, _) => other,
            };
        }
        let name = ty_ast.name.text.as_str();
        if self.types.contains_key(name) {
            return ir_ty;
        }
        match (name, ir_ty, val_ty) {
            ("Answer", ir::Type::Answer(inner), ir::Type::Answer(val_inner)) => {
                if let Some(arg_ast) = ty_ast.args.first() {
                    ir::Type::Answer(Box::new(self.fill_shades(
                        arg_ast,
                        arg_ast.arrays.len(),
                        *inner,
                        val_inner,
                    )))
                } else {
                    ir::Type::Answer(inner)
                }
            }
            (
                "Shade",
                ir::Type::Shade { origin, inner },
                ir::Type::Shade { origin: val_origin, inner: val_inner },
            ) => {
                let arg_ast = ty_ast.args.first();
                let explicit = arg_ast.is_some_and(|a| self.has_explicit_depth(a));
                let next_origin = if explicit { origin } else { *val_origin };
                let next_inner = if let Some(arg) = arg_ast {
                    self.fill_shades(arg, arg.arrays.len(), *inner, val_inner)
                } else {
                    *inner
                };
                ir::Type::Shade { origin: next_origin, inner: Box::new(next_inner) }
            }
            (_, other, _) => other,
        }
    }

    fn has_explicit_depth(&self, mut t: &'a ast::Type) -> bool {
        let mut seen: Vec<&'a str> = Vec::new();
        loop {
            if t.depth.is_some() {
                return true;
            }
            let name = t.name.text.as_str();
            if let Some((_, asserted, _, _)) = self.expanded_aliases.get(name) {
                return asserted.is_some();
            }
            if seen.contains(&name) {
                return false;
            }
            seen.push(name);
            let Some(TypeDecl::Alias(alias)) = self.types.get(name).copied() else {
                return false;
            };
            t = alias;
        }
    }

    // ── functions ───────────────────────────────────────────────────────────

    fn func(&mut self, f: &'a ast::Func, sig: &FuncSig) -> ir::FuncDef {
        self.locals = Vec::new();
        self.local_names.clear();
        self.next_temp = 0;
        self.depths = Vec::new();
        self.scopes = vec![Vec::new()];
        self.pc = Depth::PURE;
        self.ret_depth = Depth::PURE;
        self.in_loop = 0;

        let mut seen_params: Vec<&str> = Vec::with_capacity(f.params.len());
        let params: Vec<ir::LocalId> = f
            .params
            .iter()
            .zip(&sig.0)
            .map(|(p, (ty, asserted))| {
                let pname = p.name.text.as_str();
                if seen_params.contains(&pname) {
                    self.fault(p.name.span, FaultKind::Expected("a unique parameter name"));
                } else {
                    seen_params.push(pname);
                }
                // [ABS]: a parameter is bound at depth 0, and its real depth
                // arrives at the call site.
                self.bind(&p.name, ty.clone(), *asserted, Depth::PURE, p.span)
            })
            .collect();

        let (ret_ty, asserted_ret) = (sig.1.0.clone(), sig.1.1);
        self.returning = Some(ret_ty.clone());
        let body = self.block_landed(&f.body, Some(ret_ty));
        let ret = self.returning.take().unwrap_or(ir::Type::Unit);
        let tail_depth = body.tail.as_ref().map_or(Depth::PURE, |t| t.depth);
        ir::FuncDef {
            name: f.name.text.clone(),
            params,
            ret,
            ret_depth: tail_depth.join(self.ret_depth),
            asserted_ret,
            latent: Self::needs(&body),
            asserted_latent: Self::written(f.latent.as_deref()),
            locals: std::mem::take(&mut self.locals),
            body,
            span: f.span,
        }
    }

    /// The latent set written after a signature, if one was.
    ///
    /// §3.5 spells it `@0`, `@3` or `@{3,5}`. A `0` among several is refused
    /// by the parser, so a set here holds only strata that mean something.
    fn written(digits: Option<&[u8]>) -> Option<Held> {
        let digits = digits?;
        Some(digits.iter().filter_map(|n| Depth::new(*n)).fold(Held::NONE, Held::with))
    }

    /// The least set, by inclusion, at which this body checks: what an
    /// application in it asked for and the surface could not give. [ABS].
    fn needs(block: &ir::Block) -> Held {
        fn walk(x: &ir::Expr, ambient: Held, acc: Held) -> Held {
            let mut acc = acc;
            if let ir::ExprKind::Call { callee, .. } = &x.kind {
                if let ir::Type::Fn { latent, .. } = callee.ty {
                    acc = acc.union(latent.without(ambient));
                }
            }
            let inside = match &x.kind {
                ir::ExprKind::Descend { capability, .. } => ambient.with(capability.stratum()),
                _ => ambient,
            };
            ir::children(x).iter().fold(acc, |a, c| walk(c, inside, a))
        }
        block_children(block).iter().fold(Held::NONE, |acc, x| walk(x, Held::NONE, acc))
    }

    fn bind(
        &mut self,
        name: &ast::Name,
        ty: ir::Type,
        asserted: Asserted,
        depth: Depth,
        span: Span,
    ) -> ir::LocalId {
        let id = ir::LocalId(u32::try_from(self.locals.len()).unwrap_or(0));
        self.local_names.insert(name.text.clone());
        self.locals.push(ir::LocalDef { name: name.text.clone(), ty, asserted, span });
        self.depths.push(depth);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.text.clone(), id));
        }
        id
    }

    fn fresh_temp_local(&mut self, ty: ir::Type, depth: Depth, span: Span) -> ir::LocalId {
        let name = loop {
            let candidate = format!("_idx{}", self.next_temp);
            self.next_temp += 1;
            let taken = self.local_names.contains(&candidate)
                || self.globals.contains_key(candidate.as_str())
                || self.funcs.contains_key(candidate.as_str())
                || Prim::from_name(&candidate).is_some();
            if !taken {
                break candidate;
            }
        };
        let id = ir::LocalId(u32::try_from(self.locals.len()).unwrap_or(0));
        self.local_names.insert(name.clone());
        self.locals.push(ir::LocalDef { name, ty, asserted: None, span });
        self.depths.push(depth);
        id
    }

    fn lookup(&self, name: &str) -> Option<ir::LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.iter().rev().find(|(n, _)| n == name).map(|(_, id)| *id))
    }
}

fn block_children(b: &ir::Block) -> Vec<&ir::Expr> {
    let mut out: Vec<&ir::Expr> = b
        .stmts
        .iter()
        .filter_map(|s| match s {
            ir::Stmt::Let { value, .. } | ir::Stmt::Expr(value) => Some(value),
            ir::Stmt::Declare { .. } => None,
        })
        .collect();
    out.extend(b.tail.as_deref());
    out
}

// ── statements and expressions ──────────────────────────────────────────────

impl Lowering<'_> {
    fn at(kind: ir::ExprKind, ty: ir::Type, depth: Depth, span: Span) -> ir::Expr {
        ir::Expr { kind, ty, depth, span }
    }

    fn lit(l: ir::Literal, ty: ir::Type, span: Span) -> ir::Expr {
        Self::at(ir::ExprKind::Literal(l), ty, Depth::PURE, span)
    }

    fn unit_at(span: Span) -> ir::Expr {
        Self::lit(ir::Literal::Unit, ir::Type::Unit, span)
    }

    fn block(&mut self, b: &ast::Block) -> ir::Block {
        self.block_landed(b, None)
    }

    fn block_landed(&mut self, b: &ast::Block, landing: Option<ir::Type>) -> ir::Block {
        self.scopes.push(Vec::new());
        let saved_pc = self.pc;
        let mut exit = ExitPc::NONE;
        let mut stmts = Vec::new();
        for s in &b.stmts {
            self.last_exit = ExitPc::NONE;
            self.stmt(s, &mut stmts);
            let ex = self.last_exit;
            exit = exit.join(ex);
            if let Some(ex_d) = ex.deepest() {
                self.pc = self.pc.join(ex_d);
            }
        }
        let tail = b.tail.as_ref().map(|t| {
            self.last_exit = ExitPc::NONE;
            let e = self.landed(t, landing);
            exit = exit.join(self.last_exit);
            Box::new(e)
        });
        self.pc = saved_pc;
        self.last_exit = exit;
        self.scopes.pop();
        ir::Block { stmts, tail, span: b.span }
    }

    fn stmt(&mut self, s: &ast::Stmt, out: &mut Vec<ir::Stmt>) {
        match s {
            ast::Stmt::Let(binding) => {
                // No initialiser is how an aggregate is constructed: declare
                // it, then write its fields. §4.2, §5.4.
                let Some(init) = binding.value.as_ref() else {
                    let (ty, asserted, _) = self.ty_in(&binding.ty, false);
                    let local = self.bind(&binding.name, ty, asserted, Depth::PURE, binding.span);
                    out.push(ir::Stmt::Declare { local });
                    self.last_exit = ExitPc::NONE;
                    return;
                };
                let (ty, asserted, has_inferred) = self.ty_in(&binding.ty, true);
                let value = self.landed(init, (!has_inferred).then(|| ty.clone()));
                let ty = self.binding_type(&binding.ty, ty, &value);
                let depth = value.depth;
                let local = self.bind(&binding.name, ty, asserted, depth, binding.span);
                out.push(ir::Stmt::Let { local, value });
            }
            ast::Stmt::Expr(e) => {
                let value = self.expr(e);
                out.push(ir::Stmt::Expr(value));
            }
            ast::Stmt::Block(b) => {
                let block = self.block(b);
                let (ty, depth) = block
                    .tail
                    .as_ref()
                    .map_or((ir::Type::Unit, Depth::PURE), |t| (t.ty.clone(), t.depth));
                out.push(ir::Stmt::Expr(Self::at(ir::ExprKind::Block(block), ty, depth, b.span)));
            }
            ast::Stmt::If { .. } => {
                let value = self.branch(s);
                out.push(ir::Stmt::Expr(value));
            }
            ast::Stmt::While { cond, body, span } => {
                let looping = self.looping(Some(cond), None, body, *span);
                out.push(ir::Stmt::Expr(looping));
            }
            ast::Stmt::For { init, cond, step, body, span } => {
                self.for_stmt(init.as_deref(), cond.as_ref(), step.as_ref(), body, *span, out);
            }
            ast::Stmt::Return { value, span } => {
                if self.returning.is_none() {
                    self.fault(*span, FaultKind::Expected("`return` inside a function"));
                }
                let returning = self.returning.clone();
                let (value, ex) = match value.as_ref() {
                    Some(v) => {
                        let e = self.landed(v, returning);
                        (Some(Box::new(e)), self.last_exit)
                    }
                    None => (None, ExitPc::NONE),
                };
                let depth = value.as_ref().map_or(Depth::PURE, |v| v.depth);
                let ret_pc = ex.deepest().map_or(self.pc, |e| self.pc.join(e));
                self.ret_depth = self.ret_depth.join(depth).join(ret_pc);
                self.last_exit = ex.join(ExitPc { loop_exit: None, fn_exit: Some(ret_pc) });
                out.push(ir::Stmt::Expr(Self::at(
                    ir::ExprKind::Return(value),
                    ir::Type::Unit,
                    depth,
                    *span,
                )));
            }
            ast::Stmt::Break(span) => {
                if self.in_loop == 0 {
                    self.fault(*span, FaultKind::Expected("`break` inside a loop"));
                }
                self.last_exit = ExitPc { loop_exit: Some(self.pc), fn_exit: None };
                out.push(ir::Stmt::Expr(Self::at(
                    ir::ExprKind::Break,
                    ir::Type::Unit,
                    Depth::PURE,
                    *span,
                )));
            }
            ast::Stmt::Continue(span) => {
                if self.in_loop == 0 {
                    self.fault(*span, FaultKind::Expected("`continue` inside a loop"));
                }
                self.last_exit = ExitPc { loop_exit: Some(self.pc), fn_exit: None };
                out.push(ir::Stmt::Expr(Self::at(
                    ir::ExprKind::Continue,
                    ir::Type::Unit,
                    Depth::PURE,
                    *span,
                )));
            }
        }
    }

    /// `for` introduces a scope for its init clause and lowers to a loop inside it.
    fn for_stmt(
        &mut self,
        init: Option<&ast::Stmt>,
        cond: Option<&ast::Expr>,
        step: Option<&ast::Expr>,
        body: &ast::Block,
        span: Span,
        out: &mut Vec<ir::Stmt>,
    ) {
        self.scopes.push(Vec::new());
        let saved_pc = self.pc;
        let mut exit = ExitPc::NONE;
        let mut inner = Vec::new();
        if let Some(init) = init {
            self.last_exit = ExitPc::NONE;
            self.stmt(init, &mut inner);
            exit = exit.join(self.last_exit);
            if let Some(ex_d) = self.last_exit.deepest() {
                self.pc = self.pc.join(ex_d);
            }
        }
        let looping = self.looping(cond, step, body, span);
        exit = exit.join(self.last_exit);
        self.pc = saved_pc;
        self.last_exit = exit;
        self.scopes.pop();
        if inner.is_empty() {
            out.push(ir::Stmt::Expr(looping));
        } else {
            inner.push(ir::Stmt::Expr(looping));
            let block = ir::Block { stmts: inner, tail: None, span };
            out.push(ir::Stmt::Expr(Self::as_expr(block, span)));
        }
    }

    /// `if`, `else if` and `else`, all of them one branch.
    fn branch(&mut self, s: &ast::Stmt) -> ir::Expr {
        let ast::Stmt::If { cond, then, otherwise, span } = s else {
            self.last_exit = ExitPc::NONE;
            return Self::unit_at(Span::default());
        };
        let cond = self.expr(cond);
        let e_cond = self.last_exit;
        let saved_pc = self.pc;
        self.pc = e_cond.deepest().map_or(saved_pc, |e| saved_pc.join(e)).join(cond.depth);
        let before = self.depths.clone();
        let then_block = self.block(then);
        let e_then = self.last_exit;
        let then_depths = self.depths[..before.len()].to_vec();
        self.depths[..before.len()].copy_from_slice(&before);
        let then = Self::as_expr(then_block, then.span);
        let (otherwise, e_else) = match otherwise.as_deref() {
            Some(ast::Else::Block(b)) => {
                let block = self.block(b);
                (Self::as_expr(block, b.span), self.last_exit)
            }
            Some(ast::Else::If(inner)) => {
                let e = self.branch(inner);
                (e, self.last_exit)
            }
            None => (Self::unit_at(*span), ExitPc::NONE),
        };
        self.pc = saved_pc;
        self.last_exit = e_cond.join(e_then).join(e_else);
        for (slot, was) in self.depths.iter_mut().zip(then_depths) {
            *slot = slot.join(was);
        }
        let depth = cond.depth.join(then.depth).join(otherwise.depth);
        let ty = then.ty.clone();
        Self::at(
            ir::ExprKind::Select {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            ty,
            depth,
            *span,
        )
    }

    fn as_expr(block: ir::Block, span: Span) -> ir::Expr {
        let (ty, depth) =
            block.tail.as_ref().map_or((ir::Type::Unit, Depth::PURE), |t| (t.ty.clone(), t.depth));
        Self::at(ir::ExprKind::Block(block), ty, depth, span)
    }

    /// `while` and `for` are one loop. The condition becomes the guard the
    /// body opens with, and the step stays a field so that `continue` runs it.
    fn looping(
        &mut self,
        cond: Option<&ast::Expr>,
        step: Option<&ast::Expr>,
        body: &ast::Block,
        span: Span,
    ) -> ir::Expr {
        let mark = (self.locals.len(), self.faults.len());
        let saved_pc = self.pc;
        let mut loop_pc = saved_pc;
        self.in_loop += 1;
        let out = loop {
            let was = (self.depths[..mark.0].to_vec(), loop_pc);
            self.pc = loop_pc;
            let (turn, combined) = self.turn(cond, step, body, span);
            if let Some(ex) = combined.deepest() {
                loop_pc = loop_pc.join(ex);
            }
            let out_exit = ExitPc { loop_exit: None, fn_exit: combined.fn_exit };
            if self.depths[..mark.0] == was.0[..] && loop_pc == was.1 {
                self.last_exit = out_exit;
                break turn;
            }
            self.fixpoint_steps += 1;
            if self.fixpoint_steps > MAX_FIXPOINT_STEPS {
                self.faults.truncate(mark.1);
                self.fault(span, FaultKind::TooDeep);
                self.last_exit = out_exit;
                break turn;
            }
            self.locals.truncate(mark.0);
            self.depths.truncate(mark.0);
            self.faults.truncate(mark.1);
        };
        self.in_loop -= 1;
        self.pc = saved_pc;
        out
    }

    /// One turn of a loop, lowered from the depths its locals have now.
    fn turn(
        &mut self,
        cond: Option<&ast::Expr>,
        step: Option<&ast::Expr>,
        body: &ast::Block,
        span: Span,
    ) -> (ir::Expr, ExitPc) {
        let loop_pc = self.pc;
        let mut guard_exit = ExitPc::NONE;
        let guard = cond.map(|c| {
            let c = self.expr(c);
            let e_c = self.last_exit;
            let depth = c.depth;
            guard_exit = e_c.join(ExitPc { loop_exit: Some(loop_pc.join(depth)), fn_exit: None });
            let negated = Self::at(
                ir::ExprKind::Unary { op: UnOp::Not, operand: Box::new(c) },
                ir::Type::Bool,
                depth,
                span,
            );
            // A block, because `if (c) { break; }` written out is a block and
            // the two have to lower to the same thing.
            let breaking = Self::at(
                ir::ExprKind::Block(ir::Block {
                    stmts: vec![ir::Stmt::Expr(Self::at(
                        ir::ExprKind::Break,
                        ir::Type::Unit,
                        Depth::PURE,
                        span,
                    ))],
                    tail: None,
                    span,
                }),
                ir::Type::Unit,
                Depth::PURE,
                span,
            );
            ir::Stmt::Expr(Self::at(
                ir::ExprKind::Select {
                    cond: Box::new(negated),
                    then: Box::new(breaking),
                    otherwise: Box::new(Self::unit_at(span)),
                },
                ir::Type::Unit,
                depth,
                span,
            ))
        });

        if let Some(ex_d) = guard_exit.deepest() {
            self.pc = self.pc.join(ex_d);
        }
        let mut inner = self.block(body);
        let e_body = guard_exit.join(self.last_exit);
        if let Some(guard) = guard {
            inner.stmts.insert(0, guard);
        }
        let body = Self::as_expr(inner, body.span);
        let step_pc = e_body.deepest().map_or(loop_pc, |e| loop_pc.join(e));
        self.pc = step_pc;
        let (step, e_step) = match step {
            Some(s) => {
                let e = self.expr(s);
                (Some(Box::new(e)), self.last_exit)
            }
            None => (None, ExitPc::NONE),
        };
        self.pc = loop_pc;
        let depth = body.depth.join(step.as_ref().map_or(Depth::PURE, |s| s.depth));
        (
            Self::at(
                ir::ExprKind::Loop { body: Box::new(body), step },
                ir::Type::Unit,
                depth,
                span,
            ),
            e_body.join(e_step),
        )
    }
}

// ── expressions ─────────────────────────────────────────────────────────────

impl Lowering<'_> {
    fn expr(&mut self, e: &ast::Expr) -> ir::Expr {
        let span = e.span;
        let landing = self.landing.take();
        self.last_exit = ExitPc::NONE;
        match &e.kind {
            ast::ExprKind::Int(n) => Self::lit(ir::Literal::Int(*n), ir::Type::Int, span),
            ast::ExprKind::Str(s) => Self::lit(ir::Literal::Str(s.clone()), ir::Type::Str, span),
            ast::ExprKind::Bytes(b) => {
                Self::lit(ir::Literal::Bytes(b.clone()), ir::Type::Bytes, span)
            }
            ast::ExprKind::Cairn(c) => Self::lit(ir::Literal::Cairn(*c), ir::Type::Cairn, span),
            ast::ExprKind::Bool(b) => Self::lit(ir::Literal::Bool(*b), ir::Type::Bool, span),
            ast::ExprKind::Name(name) => {
                let expr = self.name(name, &[]);
                if let ir::ExprKind::Prim(p) = expr.kind
                    && is_poly_prim(p)
                {
                    self.fault(
                        name.span,
                        FaultKind::Expected("a direct call to polymorphic prelude function"),
                    );
                }
                expr
            }
            ast::ExprKind::Block(b) => {
                let block = self.block_landed(b, landing);
                Self::as_expr(block, span)
            }
            ast::ExprKind::Descend { capability, body } => {
                let Some(cap) = Capability::from_name(&capability.text) else {
                    self.fault(capability.span, FaultKind::Unknown("a capability"));
                    return Self::unit_at(span);
                };
                let block = self.block_landed(body, landing);
                let inner = Self::as_expr(block, body.span);
                let (ty, depth) = (inner.ty.clone(), inner.depth);
                Self::at(
                    ir::ExprKind::Descend { capability: cap, body: Box::new(inner) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Unary { op, operand } => {
                let operand = self.expr(operand);
                let ty = if *op == UnOp::Not { ir::Type::Bool } else { operand.ty.clone() };
                let depth = operand.depth;
                Self::at(
                    ir::ExprKind::Unary { op: *op, operand: Box::new(operand) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Rite { rite, operand } => self.rite(*rite, operand, landing, span),
            ast::ExprKind::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, span),
            ast::ExprKind::Assign { op, place, value } => self.assign(*op, place, value, span),
            ast::ExprKind::Call { callee, args } => self.call(callee, args, landing, span),
            ast::ExprKind::Index { base, index } => {
                let base = self.expr(base);
                let e_base = self.last_exit;
                let saved_pc = self.pc;
                if let Some(ed) = e_base.deepest() {
                    self.pc = saved_pc.join(ed);
                }
                let index = self.expr(index);
                self.pc = saved_pc;
                self.last_exit = e_base.join(self.last_exit);
                let ty = if let ir::Type::Array { elem, .. } = &base.ty {
                    (**elem).clone()
                } else {
                    self.fault(span, FaultKind::Unknown("an array to index"));
                    ir::Type::Unit
                };
                let depth = base.depth.join(index.depth);
                Self::at(
                    ir::ExprKind::Index { base: Box::new(base), index: Box::new(index) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Field { base, name } => {
                let base = self.expr(base);
                let (index, ty) = self.field_of(&base.ty, name);
                let depth = base.depth;
                Self::at(ir::ExprKind::Field { base: Box::new(base), index }, ty, depth, span)
            }
        }
    }

    /// An identifier: a local, a unit-level binding, a function, a prelude
    /// function, or one of the six refusal constants. In that order, so that a
    /// binding shadows a prelude name rather than colliding with it.
    fn name(&mut self, name: &ast::Name, args: &[ir::Type]) -> ir::Expr {
        let span = name.span;
        if let Some(id) = self.lookup(&name.text) {
            let (ty, depth) =
                self.locals.get(id.0 as usize).map_or((ir::Type::Unit, Depth::PURE), |l| {
                    (l.ty.clone(), self.depths[id.0 as usize])
                });
            return Self::at(ir::ExprKind::Local(id), ty, depth, span);
        }
        if let Some((id, ty, depth)) = self.globals.get(name.text.as_str()).cloned() {
            return Self::at(ir::ExprKind::Global(id), ty, depth, span);
        }
        if let Some((id, ty)) = self.funcs.get(name.text.as_str()).cloned() {
            return Self::at(ir::ExprKind::Func(id), ty, Depth::PURE, span);
        }
        if let Some(p) = Prim::from_name(&name.text) {
            return Self::at(ir::ExprKind::Prim(p), prim_arrow(p, args), Depth::PURE, span);
        }
        if let Some(r) = Refusal::from_name(&name.text) {
            return Self::lit(ir::Literal::Refusal(r), ir::Type::Refusal, span);
        }
        self.fault(span, FaultKind::UnknownName(name.text.clone()));
        Self::unit_at(span)
    }

    fn field_of(&mut self, ty: &ir::Type, name: &ast::Name) -> (u32, ir::Type) {
        let ir::Type::Struct(sname) = ty else {
            self.fault(name.span, FaultKind::Unknown("a struct to take a field from"));
            return (0, ir::Type::Unit);
        };
        let Some(TypeDecl::Struct(fields)) = self.types.get(sname.as_str()).copied() else {
            self.fault(name.span, FaultKind::Unknown("that struct"));
            return (0, ir::Type::Unit);
        };
        if let Some(i) = fields.iter().position(|f| f.name.text == name.text) {
            let ty = self.ty(&fields[i].ty);
            (u32::try_from(i).unwrap_or(0), ty)
        } else {
            self.fault(name.span, FaultKind::Unknown("a field of that name"));
            (0, ir::Type::Unit)
        }
    }

    fn rite(
        &mut self,
        rite: Rite,
        operand: &ast::Expr,
        landing: Option<ir::Type>,
        span: Span,
    ) -> ir::Expr {
        let operand =
            if rite == Rite::Opaque { self.landed(operand, landing) } else { self.expr(operand) };
        let (ty, depth) = match rite {
            Rite::Seal => (ir::Type::Cairn, Depth::PURE),
            Rite::Shade => (
                ir::Type::Shade { origin: operand.depth, inner: Box::new(operand.ty.clone()) },
                Depth::PURE,
            ),
            Rite::Look => {
                if let ir::Type::Shade { origin, inner } = &operand.ty {
                    ((**inner).clone(), origin.join(operand.depth))
                } else {
                    self.fault(span, FaultKind::Unknown("a shade to look at"));
                    (ir::Type::Unit, operand.depth)
                }
            }
            Rite::Opaque => (operand.ty.clone(), operand.depth),
        };
        Self::at(ir::ExprKind::Rite { rite, operand: Box::new(operand) }, ty, depth, span)
    }

    fn binary(&mut self, op: ast::BinOp, l: &ast::Expr, r: &ast::Expr, span: Span) -> ir::Expr {
        // `&&` and `||` short-circuit, so they are control flow rather than
        // operators and become the one branching form.
        if matches!(op, ast::BinOp::And | ast::BinOp::Or) {
            let lhs = self.expr(l);
            let e_lhs = self.last_exit;
            let saved_pc = self.pc;
            self.pc = e_lhs.deepest().map_or(saved_pc, |e| saved_pc.join(e)).join(lhs.depth);
            let rhs = self.expr(r);
            let e_rhs = self.last_exit;
            self.pc = saved_pc;
            self.last_exit = e_lhs.join(e_rhs);
            let constant = ir::Literal::Bool(op == ast::BinOp::Or);
            let short =
                Self::at(ir::ExprKind::Literal(constant), ir::Type::Bool, Depth::PURE, span);
            let depth = lhs.depth.join(rhs.depth);
            let (then, otherwise) = if op == ast::BinOp::And { (rhs, short) } else { (short, rhs) };
            return Self::at(
                ir::ExprKind::Select {
                    cond: Box::new(lhs),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                },
                ir::Type::Bool,
                depth,
                span,
            );
        }
        let lhs = self.expr(l);
        let e_lhs = self.last_exit;
        let saved_pc = self.pc;
        if let Some(ed) = e_lhs.deepest() {
            self.pc = saved_pc.join(ed);
        }
        let rhs = self.expr(r);
        self.pc = saved_pc;
        self.last_exit = e_lhs.join(self.last_exit);
        let op = arithmetic(op);
        let ty = if comparison(op) { ir::Type::Bool } else { lhs.ty.clone() };
        let depth = lhs.depth.join(rhs.depth);
        Self::at(
            ir::ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
            ty,
            depth,
            span,
        )
    }

    fn assign(
        &mut self,
        op: Option<ast::BinOp>,
        target: &ast::Expr,
        value: &ast::Expr,
        span: Span,
    ) -> ir::Expr {
        let saved_pc = self.pc;
        let Some((place, place_ty)) = self.place(target) else {
            self.pc = saved_pc;
            self.fault(target.span, FaultKind::NotAPlace);
            return Self::unit_at(span);
        };
        let e_place = self.last_exit;
        if let Some(ed) = e_place.deepest() {
            self.pc = self.pc.join(ed);
        }
        // `x op= e` reads the place and writes back `place op value`.
        // Hoist any index expressions in `place.path` into hidden locals so
        // side-effecting index expressions are lowered and evaluated once.
        let (stmts, assign_place, val_expr, e_val) = if let Some(op) = op {
            let mut stmts = Vec::new();
            let mut read_path = Vec::with_capacity(place.path.len());
            let mut write_path = Vec::with_capacity(place.path.len());
            for proj in place.path {
                match proj {
                    ir::Proj::Field(idx) => {
                        read_path.push(ir::Proj::Field(idx));
                        write_path.push(ir::Proj::Field(idx));
                    }
                    ir::Proj::Index(idx) => {
                        if matches!(idx.kind, ir::ExprKind::Literal(_)) {
                            read_path.push(ir::Proj::Index(idx.clone()));
                            write_path.push(ir::Proj::Index(idx));
                        } else {
                            let (idx_span, idx_ty, idx_depth) =
                                (idx.span, idx.ty.clone(), idx.depth);
                            let tmp = self.fresh_temp_local(idx_ty.clone(), idx_depth, idx_span);
                            stmts.push(ir::Stmt::Let { local: tmp, value: *idx });
                            let load =
                                Self::at(ir::ExprKind::Local(tmp), idx_ty, idx_depth, idx_span);
                            read_path.push(ir::Proj::Index(Box::new(load.clone())));
                            write_path.push(ir::Proj::Index(Box::new(load)));
                        }
                    }
                }
            }
            let lhs = self.read_place(place.local, &read_path, target.span);
            let rhs = self.expr(value);
            let e_rhs = self.last_exit;
            let bin_op = arithmetic(op);
            let bin_ty = if comparison(bin_op) { ir::Type::Bool } else { lhs.ty.clone() };
            let bin_depth = lhs.depth.join(rhs.depth);
            let combined = Self::at(
                ir::ExprKind::Binary { op: bin_op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                bin_ty,
                bin_depth,
                span,
            );
            (stmts, ir::Place { local: place.local, path: write_path }, combined, e_rhs)
        } else {
            let value = self.landed(value, Some(place_ty));
            (Vec::new(), place, value, self.last_exit)
        };
        if let Some(ed) = e_val.deepest() {
            self.pc = self.pc.join(ed);
        }
        let depth = assign_place.path.iter().fold(val_expr.depth, |acc, p| match p {
            ir::Proj::Index(i) => acc.join(i.depth),
            ir::Proj::Field(_) => acc,
        });
        if let Some(slot) = self.depths.get_mut(assign_place.local.0 as usize) {
            *slot = slot.join(depth).join(self.pc);
        }
        self.pc = saved_pc;
        self.last_exit = e_place.join(e_val);
        let assign_expr = Self::at(
            ir::ExprKind::Assign { place: assign_place, value: Box::new(val_expr) },
            ir::Type::Unit,
            depth,
            span,
        );
        if stmts.is_empty() {
            assign_expr
        } else {
            Self::as_expr(ir::Block { stmts, tail: Some(Box::new(assign_expr)), span }, span)
        }
    }

    fn read_place(&mut self, local: ir::LocalId, path: &[ir::Proj], span: Span) -> ir::Expr {
        let (mut cur_ty, mut cur_depth) =
            self.locals.get(local.0 as usize).map_or((ir::Type::Unit, Depth::PURE), |l| {
                (l.ty.clone(), self.depths[local.0 as usize])
            });
        let mut expr = Self::at(ir::ExprKind::Local(local), cur_ty.clone(), cur_depth, span);
        for proj in path {
            match proj {
                ir::Proj::Field(idx) => {
                    let field_ty = if let ir::Type::Struct(sname) = &cur_ty
                        && let Some(TypeDecl::Struct(fields)) =
                            self.types.get(sname.as_str()).copied()
                        && let Some(f) = fields.get(*idx as usize)
                    {
                        self.ty(&f.ty)
                    } else {
                        ir::Type::Unit
                    };
                    cur_ty = field_ty.clone();
                    expr = Self::at(
                        ir::ExprKind::Field { base: Box::new(expr), index: *idx },
                        field_ty,
                        cur_depth,
                        span,
                    );
                }
                ir::Proj::Index(idx) => {
                    let elem_ty = if let ir::Type::Array { elem, .. } = cur_ty {
                        *elem
                    } else {
                        ir::Type::Unit
                    };
                    cur_depth = cur_depth.join(idx.depth);
                    cur_ty = elem_ty.clone();
                    expr = Self::at(
                        ir::ExprKind::Index { base: Box::new(expr), index: idx.clone() },
                        elem_ty,
                        cur_depth,
                        span,
                    );
                }
            }
        }
        expr
    }

    /// The left of an assignment: a local, and a path of fields and indices
    /// from it, together with the type at the tip. Single-pass. §5.2.
    fn place(&mut self, e: &ast::Expr) -> Option<(ir::Place, ir::Type)> {
        self.last_exit = ExitPc::NONE;
        match &e.kind {
            ast::ExprKind::Name(name) => {
                let local = self.lookup(&name.text)?;
                let ty = self.locals.get(local.0 as usize).map_or(ir::Type::Unit, |l| l.ty.clone());
                Some((ir::Place { local, path: Vec::new() }, ty))
            }
            ast::ExprKind::Field { base, name } => {
                let (mut place, base_ty) = self.place(base)?;
                let (index, field_ty) = self.field_of(&base_ty, name);
                place.path.push(ir::Proj::Field(index));
                Some((place, field_ty))
            }
            ast::ExprKind::Index { base, index } => {
                let (mut place, base_ty) = self.place(base)?;
                let e_base = self.last_exit;
                let saved_pc = self.pc;
                if let Some(ed) = e_base.deepest() {
                    self.pc = saved_pc.join(ed);
                }
                let elem_ty = if let ir::Type::Array { elem, .. } = base_ty {
                    *elem
                } else {
                    self.fault(e.span, FaultKind::Unknown("an array to index"));
                    ir::Type::Unit
                };
                let index = self.expr(index);
                self.pc = saved_pc;
                self.last_exit = e_base.join(self.last_exit);
                place.path.push(ir::Proj::Index(Box::new(index)));
                Some((place, elem_ty))
            }
            _ => None,
        }
    }

    /// An expression that is the whole of something with a declared type.
    fn landed(&mut self, e: &ast::Expr, ty: Option<ir::Type>) -> ir::Expr {
        self.landing = ty;
        self.expr(e)
    }

    fn call(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Expr],
        landing: Option<ir::Type>,
        span: Span,
    ) -> ir::Expr {
        enum PendingCallee<'b> {
            Name(&'b ast::Name),
            Expr(ir::Expr),
        }
        let saved_pc = self.pc;
        let mut ex = ExitPc::NONE;
        // Evaluate non-name callees BEFORE arguments so evaluation order matches
        // `nether-core::check` (callee left-to-right before args).
        let (param_types, pre_callee) = match &callee.kind {
            ast::ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                let ps = match self.funcs.get(name.text.as_str()) {
                    Some((_, ir::Type::Fn { params, .. })) => Some(params.clone()),
                    _ => None,
                };
                (ps, PendingCallee::Name(name))
            }
            ast::ExprKind::Name(name) => (None, PendingCallee::Name(name)),
            _ => {
                let c = self.expr(callee);
                ex = ex.join(self.last_exit);
                if let Some(ed) = self.last_exit.deepest() {
                    self.pc = self.pc.join(ed);
                }
                let ps = match &c.ty {
                    ir::Type::Fn { params, .. } => Some(params.clone()),
                    _ => None,
                };
                (ps, PendingCallee::Expr(c))
            }
        };
        let mut lowered_args = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            let expected = param_types.as_ref().and_then(|ps| ps.get(i).cloned());
            let arg = self.landed(a, expected);
            ex = ex.join(self.last_exit);
            if let Some(ed) = self.last_exit.deepest() {
                self.pc = self.pc.join(ed);
            }
            lowered_args.push(arg);
        }
        self.pc = saved_pc;
        let types: Vec<ir::Type> = lowered_args.iter().map(|a| a.ty.clone()).collect();
        let faults_before = self.faults.len();
        let mut callee = match pre_callee {
            PendingCallee::Expr(c) => c,
            PendingCallee::Name(name) => self.name(name, &types),
        };
        let name_failed = self.faults.len() > faults_before;
        self.last_exit = ex;
        // §9.2: validate polymorphic prelude arity and argument shapes.
        match callee.kind {
            ir::ExprKind::Prim(Prim::Refused) => {
                if lowered_args.len() != 1 || types.first() != Some(&ir::Type::Refusal) {
                    self.fault(span, FaultKind::Expected("one `Refusal` argument to `refused`"));
                }
                match (landing, &mut callee.ty) {
                    (Some(t @ ir::Type::Answer(_)), ir::Type::Fn { result, .. }) => **result = t,
                    _ => self
                        .fault(span, FaultKind::Expected("a declared `Answer<T>` for `refused`")),
                }
            }
            ir::ExprKind::Prim(Prim::Answered) if lowered_args.len() != 1 => {
                self.fault(span, FaultKind::Expected("one argument to `answered`"));
            }
            ir::ExprKind::Prim(Prim::Must | Prim::Given | Prim::Refusal)
                if lowered_args.len() != 1
                    || !matches!(types.first(), Some(ir::Type::Answer(_))) =>
            {
                self.fault(span, FaultKind::Expected("one `Answer<T>` argument"));
            }
            _ => {}
        }
        let (ty, result_depth) =
            if let ir::Type::Fn { params, result, result_depth, .. } = &callee.ty {
                if lowered_args.len() != params.len() {
                    self.fault(span, FaultKind::Expected("the right number of arguments"));
                }
                ((**result).clone(), *result_depth)
            } else {
                if !name_failed {
                    self.fault(span, FaultKind::Unknown("a function to apply"));
                }
                (ir::Type::Unit, Depth::PURE)
            };
        // [APP]: the maximum of what comes back, what the function value cost,
        // and what the arguments cost. The latent depth is the premise, not a
        // term, which is the checker's business rather than this one's.
        let depth =
            lowered_args.iter().fold(result_depth.join(callee.depth), |acc, a| acc.join(a.depth));
        Self::at(
            ir::ExprKind::Call { callee: Box::new(callee), args: lowered_args },
            ty,
            depth,
            span,
        )
    }
}

fn comparison(op: ir::BinOp) -> bool {
    matches!(
        op,
        ir::BinOp::Eq
            | ir::BinOp::Ne
            | ir::BinOp::Lt
            | ir::BinOp::Le
            | ir::BinOp::Gt
            | ir::BinOp::Ge
    )
}

fn arithmetic(op: ast::BinOp) -> ir::BinOp {
    match op {
        ast::BinOp::Mul => ir::BinOp::Mul,
        ast::BinOp::Div => ir::BinOp::Div,
        ast::BinOp::Rem => ir::BinOp::Rem,
        ast::BinOp::Add => ir::BinOp::Add,
        ast::BinOp::Sub => ir::BinOp::Sub,
        ast::BinOp::Shl => ir::BinOp::Shl,
        ast::BinOp::Shr => ir::BinOp::Shr,
        ast::BinOp::Lt => ir::BinOp::Lt,
        ast::BinOp::Le => ir::BinOp::Le,
        ast::BinOp::Gt => ir::BinOp::Gt,
        ast::BinOp::Ge => ir::BinOp::Ge,
        ast::BinOp::Eq => ir::BinOp::Eq,
        ast::BinOp::Ne => ir::BinOp::Ne,
        ast::BinOp::BitXor => ir::BinOp::BitXor,
        ast::BinOp::BitOr => ir::BinOp::BitOr,
        // `&&` and `||` never reach here: they are a branch and not an
        // operator, and `binary` takes them before this is called.
        ast::BinOp::BitAnd | ast::BinOp::And | ast::BinOp::Or => ir::BinOp::BitAnd,
    }
}

/// The prelude, instantiated. `spec/09-prelude.md`.
///
/// The names and the strata are `nether_core::Prim`'s; the shapes are here,
/// because three of them are polymorphic in `T` and the IR's types have no
/// variables. `T` comes from what the call was handed.
fn prim_arrow(p: Prim, args: &[ir::Type]) -> ir::Type {
    use ir::Type as T;
    let answer = |t: T| T::Answer(Box::new(t));
    let strs = || T::Array { elem: Box::new(T::Str), len: None };
    let held = || match args.first() {
        Some(T::Answer(inner)) => (**inner).clone(),
        _ => T::Unit,
    };
    let (params, result) = match p {
        Prim::Min | Prim::Max => (vec![T::Int, T::Int], T::Int),
        Prim::Abs => (vec![T::Int], T::Int),
        Prim::Len => (vec![T::Bytes], T::Int),
        Prim::Slice => (vec![T::Bytes, T::Int, T::Int], T::Bytes),
        Prim::Concat => (vec![T::Bytes, T::Bytes], T::Bytes),
        Prim::StartsWith => (vec![T::Bytes, T::Bytes], T::Bool),
        Prim::Utf8 => (vec![T::Bytes], answer(T::Str)),
        Prim::Raw => (vec![T::Str], T::Bytes),
        Prim::Join => (vec![strs(), T::Str], T::Str),
        Prim::Split => (vec![T::Str, T::Str], strs()),
        Prim::CairnOf => (vec![T::Bytes], T::Cairn),
        Prim::Hex => (vec![T::Cairn], T::Str),
        Prim::Given => (vec![answer(held())], T::Bool),
        Prim::Refusal => (vec![answer(held())], T::Refusal),
        Prim::Must => (vec![answer(held())], held()),
        Prim::Answered => (args.to_vec(), answer(args.first().cloned().unwrap_or(T::Unit))),
        Prim::Refused => (vec![T::Refusal], answer(T::Unit)),
        Prim::FetchNode => (vec![T::Cairn], answer(T::Bytes)),
        Prim::HasNode => (vec![T::Cairn], T::Bool),
        Prim::Env => (vec![T::Str], answer(T::Str)),
        Prim::Clock => (Vec::new(), T::Int),
        Prim::Target => (Vec::new(), T::Str),
        // The same shape from two different strata, which is the whole of the
        // difference between reading a file and fetching a page.
        Prim::Read | Prim::Get => (vec![T::Str], answer(T::Bytes)),
        Prim::List => (vec![T::Str], answer(strs())),
        Prim::Exists => (vec![T::Str], T::Bool),
        Prim::Write => (vec![T::Str, T::Bytes], answer(T::Unit)),
        Prim::Remove => (vec![T::Str], answer(T::Unit)),
        Prim::Post | Prim::CallForeign => (vec![T::Str, T::Bytes], answer(T::Bytes)),
        Prim::Draw => (vec![T::Int], T::Bytes),
    };
    // Every prelude function hands back what it went and got, so its two
    // depths are the same one.
    T::Fn {
        params,
        latent: Held::of(p.latent()),
        result: Box::new(result),
        result_depth: p.latent(),
    }
}
