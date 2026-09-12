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

let_decl      := type identifier "=" expr ";" ;
```

A `let_decl` at unit level is a declaration; the same production inside a
block is a statement. Both are immutable bindings: there is no assignment to a
binding after its declaration, only to the fields of a local aggregate before
it escapes. See [section 05](05-types.md#54-mutation).

## 4.3 Types

```ebnf
type          := type_atom { "[" [ int_literal ] "]" } [ "@" digit ] ;

type_atom     := identifier
               | identifier "<" type { "," type } ">" ;
```

`Shade<Bytes>@0` is a shade of `Bytes` at value depth 0. The origin depth of a
shade is part of its type but is not written in source syntax; it is always
inferred, because writing it would let a programmer claim an origin the value
does not have.

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
