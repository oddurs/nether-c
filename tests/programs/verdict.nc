// verdict.nc — a function that says no the way the prelude does. §9.2.

Answer<Bytes> first(Bytes b)
{
  if (len(b) == 0) {
    return refused(absent);
  }
  return answered(slice(b, 0, 1));
}

U0 report()
{
  Answer<Bytes> a = first(b"nether");
  Answer<Bytes> none = first(b"");
  if (given(a)) { concat(must(a), b"\n"); }
  if (refusal(none) == absent) { "nothing there\n"; }
}

demand report();
