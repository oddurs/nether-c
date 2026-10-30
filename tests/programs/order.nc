// order.nc — the order a burial leaves, which a parallel one must leave too.
// spec/06-evaluation.md §6.2.1.
//
// `b` is declared first and `a` is asked for first, so the holes are in the
// order the demand reached them and not the order they were written. The
// deposits are in demand order, with a starved demand between them.
Bytes@3 b = must(descend disk { read("b.txt") });
Bytes@3 a = must(descend disk { read("a.txt") });

U0 first() { "one\n"; }
U0 second() { "two\n"; }

demand first();
demand concat(a, b);
demand second();
