// build.nc — spec/06-evaluation.md §6.2, and what it leans on.
//
// §9.2: `compile` is not in the prelude. Nether C does not know how to
// compile Nether C until Self-Burial; until then it is an ordinary function a
// program supplies for itself. So this one does, and `other` with it.

Bytes compile(Bytes s) { concat(b"obj:", s) }
Bytes other = b"lib.nc";

Bytes@3 src = must(descend disk { read("main.nc") });
Bytes   obj = compile(src);      // pure, but starves: src is a hole
Bytes   unused = compile(other); // never evaluated: nothing demands it

demand obj;
