// stamp.nc — the Orpheus rule, and the error spec/01-strata.md §1.6 prints.
//
// A shade may be carried up out of any stratum. It may only be looked at by
// going back down, and this does not: the look is at the top level of a file,
// where the ambient depth is 0 and the value came from stratum 5.
//
// The five lines below are §1.6's sample, unchanged, so that the file the
// error names is a file that exists and the line and column it prints are
// checked against it.
Shade<Bytes> reply = descend net { shade must(get("https://example.invalid/index.json")) };

Cairn witness = seal reply;       // legal: Cairn@0
// ILLEGAL here — see below
I64   n       = len(look(reply));
