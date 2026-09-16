---
id: 103
title: 'A pilot: build something real with it'
type: chore
status: unmarked
milestone: after
depends_on:
- 82
created: 2026-09-11
updated: 2026-09-15
priority: p0
effort: xl
area: lib/
stratum: '4'
proof: Nether C builds this repository's own site, end to end, and the result is byte-identical to site/bake's
---

Every language demo is a fibonacci function and every language demo is a lie.

The honest test is to take something that already works, rebuild it in Nether
C, and see what breaks. `site/bake` is the right target: it reads a directory,
transforms text, writes files, and its output is already checked byte-for-byte
in CI. If Nether C cannot express it, that is worth knowing before anyone else
finds out.

Expect this to produce more bug reports than any other item on the roadmap.
That is the point of it.

## Delivery plan — 2026-09-15

### Starting point and scope

The pilot compares a Nether C implementation with site/bake, not another toy example. The current interpreter subset is insufficient; preserve the 0082 milestone gate pending an explicit sequencing decision.

### Steps

1. Inventory site/bake inputs, output files, escaping and failure behavior; pin a fixture tree and oracle commit.
2. Port one end-to-end document first, filing missing specified language features as prerequisites, then extend to the full site.
3. Compare output filenames and bytes in isolated directories, including malformed input and deterministic repeat burial.

### Acceptance and evidence

- [ ] The full site matches the pinned baker byte-for-byte with recorded world inputs. Keep Python as the oracle until equivalence passes; unrelated GIF/font generation is not silently included in the baker contract.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
