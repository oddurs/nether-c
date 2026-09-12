---
section: "04"
title: Grammar
status: draft
---

# Grammar

EBNF. `{ x }` is zero or more, `[ x ]` is optional, `|` is alternation.
Terminals are quoted. Lexical productions are in
[section 03](03-lexical.md).

The grammar is recursive and an implementation is not. How deeply an
expression or a type may nest is one of the limits
[§6.4](06-evaluation.md#64-starvation-and-fuel) requires an implementation to
state, and to report reaching rather than crash into.

## 4.1 Compilation unit

```ebnf
unit          := { item } ;

item          := struct_decl
               | typedef_decl
               | func_decl
               | let_decl
               | demand_stmt ;

demand_stmt   := "demand" expr ";" ;
```

There is no entry point. A unit is a set of declarations plus the demands that
give some of them a reason to be evaluated.

## 4.2 Declarations

```ebnf
struct_decl   := "struct" identifier "{" { field } "}" ";" ;
field         := type identifier ";" ;

typedef_decl  := "typedef" type identifier ";" ;

func_decl     := type identifier "(" [ params ] ")" [ latent ] block ;
params        := param { "," param } ;
param         := type identifier ;
latent        := "@" digit ;

let_decl      := type identifier [ "=" expr ] ";" ;
```

A `let_decl` at unit level is a declaration; the same production inside a block
is a statement. A unit-level one MUST have its initialiser: a global with no
value is a value nothing can supply, since there is no statement above it to
assign one.

A block-level one may go without, and that is the only way to construct an
aggregate: declare it, then write its fields. What may be written, and for how
long, is [§5.4](05-types.md#54-mutation).

## 4.3 Types

```ebnf
type          := type_atom { "[" [ int_literal ] "]" } [ "@" digit ] ;

type_atom     := identifier
               | identifier "<" type { "," type } ">" ;
```

`Shade<Bytes>@0` is a shade of `Bytes` at value depth 0. The two depths on a
shade are different things and it is worth being careful: the one after the
`>` is how deep the *shade* is, and the one inside is how deep the value it
holds came from.

A shade's **origin** is the depth written on its type argument.
`Shade<Bytes@5>` is a shade of bytes that came from stratum 5, and
`Shade<Bytes@5>@0` is that shade held at depth 0, which is the ordinary case
and what makes a shade worth having.

Where a shade is constructed, the origin is **inferred and may not be
written**: `shade e` takes its origin from the depth of `e`, and letting a
programmer write one there would let them claim an origin the value does not
have.

Where a shade is **received** — a parameter, a struct field, a return type —
the origin MUST be written. There is nothing to infer it from, and the claim is
not believed: it is checked at every call site, which is the opposite of the
situation the rule above guards against.

```c
// Takes a shade from stratum 5. Its name may be carried anywhere; opening it
// needs `descend net`, wherever the caller is.
Cairn keep(Shade<Bytes@5> s) @0 { seal s }
```

That is the whole of it. No new syntax: the depth annotation on a type already
means "this value came from there", and a shade's argument is a type.

## 4.4 Statements

```ebnf
block         := "{" { stmt } [ expr ] "}" ;

stmt          := let_decl
               | expr ";"
               | "if" "(" expr ")" block [ "else" ( block | if_stmt ) ]
               | "while" "(" expr ")" block
               | "for" "(" [ let_decl | expr ] ";" [ expr ] ";" [ expr ] ")" block
               | "return" [ expr ] ";"
               | "break" ";"
               | "continue" ";"
               | block ;
```

A block's optional trailing `expr` — with no semicolon — is its **tail
value**. A block with no tail value has type `U0`.

## 4.5 Expressions

```ebnf
expr          := assign ;

assign        := logical_or [ assign_op assign ] ;
assign_op     := "=" | "+=" | "-=" | "*=" | "/=" | "%="
               | "&=" | "|=" | "^=" | "<<=" | ">>=" ;

logical_or    := logical_and { "||" logical_and } ;
logical_and   := bit_or      { "&&" bit_or } ;
bit_or        := bit_xor     { "|"  bit_xor } ;
bit_xor       := bit_and     { "^"  bit_and } ;
bit_and       := equality    { "&"  equality } ;
equality      := relational  { ( "==" | "!=" ) relational } ;
relational    := shift       { ( "<" | "<=" | ">" | ">=" ) shift } ;
shift         := additive    { ( "<<" | ">>" ) additive } ;
additive      := multiply    { ( "+" | "-" ) multiply } ;
multiply      := unary       { ( "*" | "/" | "%" ) unary } ;

unary         := ( "-" | "!" | "~" ) unary
               | "seal"   unary
               | "shade"  unary
               | "look"   unary
               | "opaque" unary
               | postfix ;

postfix       := primary { call_suffix | index_suffix | field_suffix } ;
call_suffix   := "(" [ args ] ")" ;
index_suffix  := "[" expr "]" ;
field_suffix  := "." identifier ;
args          := expr { "," expr } ;

primary       := int_literal
               | str_literal
               | bytes_literal
               | bool_literal
               | identifier
               | descend_expr
               | block
               | "(" expr ")" ;

descend_expr  := "descend" identifier block ;
```

## 4.6 Precedence

Highest to lowest. All binary operators are left-associative; assignment and
the unary rites are right-associative.

| Level | Operators |
| ---: | --- |
| 1 | `()` `[]` `.` |
| 2 | `-` `!` `~` `seal` `shade` `look` `opaque` (unary) |
| 3 | `*` `/` `%` |
| 4 | `+` `-` |
| 5 | `<<` `>>` |
| 6 | `<` `<=` `>` `>=` |
| 7 | `==` `!=` |
| 8 | `&` |
| 9 | `^` |
| 10 | `\|` |
| 11 | `&&` |
| 12 | `\|\|` |
| 13 | `=` and the compound assignments |

`seal read(p)` therefore seals the result of the call, not the function.

## 4.7 The bare-expression statement

```c
"Hello from the nether\n";
```

An expression statement whose value is not `U0` **deposits** that value into
the trace, tagged with its source span. It is not printed; nothing in Nether C
is printed. See [§6.8](06-evaluation.md#68-what-burial-prints).

This is the direct inversion of HolyC, in which a bare string is a call to
`PrintF`. The syntax is the same gesture; the semantics are its opposite.

An implementation MUST NOT warn about a discarded value in this position. It
is not discarded.
