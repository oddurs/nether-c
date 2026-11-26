//! Type lowering, alias expansion, struct cycle detection, shade filling, and
//! prelude signatures.

use nether_core::{self as ir, Asserted, Depth, Held, Prim, Span};

use crate::ast;
use crate::lex::FaultKind;
use crate::lower::Lowering;

/// Upper bound on expanded type nesting depth (matching `parse::MAX_NESTING`).
const MAX_TYPE_DEPTH: usize = 64;

#[derive(Clone, Copy)]
pub(crate) enum TypeDecl<'a> {
    Alias(&'a ast::Type),
    Struct(&'a [ast::Field]),
}

pub(crate) struct FuncSig {
    pub(crate) params: Vec<(ir::Type, Asserted)>,
    pub(crate) ret: ir::Type,
    pub(crate) asserted_ret: Asserted,
}

#[derive(Clone)]
pub(crate) struct LoweredType {
    pub(crate) ty: ir::Type,
    pub(crate) asserted: Asserted,
    pub(crate) has_inferred: bool,
    pub(crate) height: usize,
}

impl LoweredType {
    const fn unit(height: usize) -> Self {
        Self { ty: ir::Type::Unit, asserted: None, has_inferred: false, height }
    }
}

impl<'a> Lowering<'a> {
    /// An `ast::Type`, with aliases expanded and prelude names resolved.
    pub(crate) fn ty(&mut self, t: &ast::Type) -> ir::Type {
        self.ty_depth(t, false).ty
    }

    /// Lower an `ast::Type`. When `allow_inferred_shade` is true (initialized
    /// `let` bindings), direct `Shade<T>` annotations may omit `@d` and inherit
    /// the RHS origin.
    pub(crate) fn ty_depth(&mut self, t: &ast::Type, allow_inferred_shade: bool) -> LoweredType {
        let mut lowered = self.atom_in(t, allow_inferred_shade);
        for len in t.arrays.iter().rev() {
            lowered.height = lowered.height.saturating_add(1);
            if lowered.height > MAX_TYPE_DEPTH {
                self.fault(t.span, FaultKind::TooDeep);
                return LoweredType::unit(MAX_TYPE_DEPTH);
            }
            let len = match len {
                Some(n) if *n < 0 => {
                    self.fault(t.span, FaultKind::Expected("a non-negative array length"));
                    None
                }
                Some(n) => u64::try_from(*n).ok(),
                None => None,
            };
            lowered.ty = ir::Type::Array { elem: Box::new(lowered.ty), len };
        }
        let own = t.depth.and_then(Depth::new);
        if let (Some(a), Some(b)) = (own, lowered.asserted)
            && a != b
        {
            self.fault(t.span, FaultKind::Expected("a matching depth annotation on typedef"));
        }
        lowered.asserted = own.or(lowered.asserted);
        lowered
    }

    fn expand_alias(&mut self, key: &'a str, alias: &'a ast::Type, span: Span) -> LoweredType {
        if let Some(cached) = self.expanded_aliases.get(key) {
            return cached.clone();
        }
        if self.expanding.contains(&key) || self.expanding.len() > 16 {
            self.fault(span, FaultKind::Expected("a non-cyclic typedef"));
            return LoweredType::unit(1);
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

    fn lower_type_arg(
        &mut self,
        t: &ast::Type,
        allow_inferred_shade: bool,
        missing: &'static str,
    ) -> Result<(LoweredType, Span), LoweredType> {
        let Some(arg_ast) = t.args.first() else {
            self.fault(t.span, FaultKind::Expected(missing));
            return Err(LoweredType::unit(1));
        };
        let mut arg = self.ty_depth(arg_ast, allow_inferred_shade);
        arg.height = arg.height.saturating_add(1);
        if arg.height > MAX_TYPE_DEPTH {
            self.fault(t.span, FaultKind::TooDeep);
            return Err(LoweredType::unit(MAX_TYPE_DEPTH));
        }
        Ok((arg, arg_ast.span))
    }

    fn atom_in(&mut self, t: &ast::Type, allow_inferred_shade: bool) -> LoweredType {
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
                TypeDecl::Struct(_) => LoweredType {
                    ty: ir::Type::Struct(name.to_string()),
                    asserted: None,
                    has_inferred: false,
                    height: 1,
                },
            };
        }
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
                let (arg, arg_span) = match self.lower_type_arg(
                    t,
                    allow_inferred_shade,
                    "`Answer<T>` to say what it answers",
                ) {
                    Ok(v) => v,
                    Err(fallback) => return fallback,
                };
                if arg.asserted.is_some() {
                    self.fault(
                        arg_span,
                        FaultKind::Expected(
                            "no depth annotation inside `Answer<T>` (write `Answer<T>@d` instead)",
                        ),
                    );
                }
                return LoweredType {
                    ty: ir::Type::Answer(Box::new(arg.ty)),
                    asserted: None,
                    has_inferred: arg.has_inferred,
                    height: arg.height,
                };
            }
            // §4.3: a received shade MUST write its origin on its argument (`Shade<Bytes@5>`).
            // An initialized binding may omit it (`Shade<Bytes>`) and inherit it from the value.
            "Shade" => {
                reject_args(self, 1);
                let (arg, _) = match self.lower_type_arg(
                    t,
                    allow_inferred_shade,
                    "`Shade<T>` to say what it hides",
                ) {
                    Ok(v) => v,
                    Err(fallback) => return fallback,
                };
                let (origin, has_inferred) = if let Some(d) = arg.asserted {
                    (d, arg.has_inferred)
                } else {
                    if !allow_inferred_shade {
                        self.fault(
                            t.span,
                            FaultKind::Expected("an explicit `@d` origin on `Shade<T@d>`"),
                        );
                    }
                    (Depth::UNRECORDED, true)
                };
                return LoweredType {
                    ty: ir::Type::Shade { origin, inner: Box::new(arg.ty) },
                    asserted: None,
                    has_inferred,
                    height: arg.height,
                };
            }
            _ => {
                reject_args(self, 0);
                self.fault(t.span, FaultKind::Unknown("a type"));
                ir::Type::Unit
            }
        };
        LoweredType { ty, asserted: None, has_inferred: false, height: 1 }
    }

    pub(crate) fn check_struct_cycles(&mut self, structs: &[ir::StructDef]) {
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

    /// The declared type of a binding, recursively filling only omitted shade
    /// origins directly written on the binding (never overwriting `typedef` origins). §4.3.
    pub(crate) fn binding_type(
        &self,
        ty_ast: &ast::Type,
        ty: ir::Type,
        value: &ir::Expr,
    ) -> ir::Type {
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
            if let Some(cached) = self.expanded_aliases.get(name) {
                return cached.asserted.is_some();
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
}

/// The prelude, instantiated. `spec/09-prelude.md`.
///
/// The names and the strata are `nether_core::Prim`'s; the shapes are here,
/// because five of them are polymorphic in `T` and the IR's types have no
/// variables. `T` comes from what the call was handed.
pub(crate) fn prim_arrow(p: Prim, args: &[ir::Type]) -> ir::Type {
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
