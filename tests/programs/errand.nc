// errand.nc — the manual's worked example. docs/manual/03-the-hole.md.
//
// One read of one file, and a greeting built from what comes back. It is the
// smallest program that has a hole in it: `read` is stratum 3, burial has no
// disk, so the call cannot happen and the trace says so instead.

Bytes@3 name = must(descend disk { read("who.txt") });

U0 greet()
{
  concat(b"Hello, ", name);
}

demand greet();
