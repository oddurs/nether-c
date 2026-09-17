// turn.nc — a loop body comes after itself. spec/05-types.md §5.4.
//
// `h` is assigned above the `seal` on the page and after it on the second
// turn, so sealing it inside the body names it for the whole body. Refusing
// this is the whole of what §5.4 exists for: the cairn `k` holds would name
// bytes the next turn changed.
//
// Nothing here is deeper than the surface. It is refused by `check`, not by
// burial, and `for (I64 i = 0; i < n; i += 1)` stays legal beside it.

U0 turn(Bool c)
{
  Bytes h;
  while (c) { h = b"y"; Cairn k = seal h; }
}
