---
id: 166
title: A symlink reaches out of the disk root
type: bug
status: unmarked
milestone: world
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: crates/nether-world
stratum: '3'
proof: A file outside the root cannot be read through anything inside it
---

## Measured

```
reading through the link: Given(Bytes([83, 69, 67, 82, 69, 84]))
```

A file outside the root, read through a symbolic link inside it. The root is
what makes granting `disk` to a burial safe at all, and it does not hold.

`Disk::resolve` walks the path's components and refuses `..`, a root component
and a prefix. That stops a path climbing out textually. It never asks the
filesystem where anything actually is, so `fs::read` follows a link and lands
wherever the link points.

## The comment made it worse

> `..` is resolved textually rather than by asking the filesystem, because
> asking would follow a symbolic link back out of the root.

Resolving textually is *why* the link is followed. The sentence names the exact
attack and reads as though it were handled, which is the kind of comment that
stops the next person looking.

## What to do

Keep the textual pass — it is what refuses `..` on a path that does not exist
yet — and then ask. The deepest ancestor of the target that exists has to
canonicalise to something still under a canonicalised root.

And say what that is worth. It is a check before an act, so a link created
between the two still wins; what it stops is a link that was already there,
which is the case a program being buried can arrange for itself. Claiming more
than that is how this got here.

## Acceptance criteria

- [ ] A file outside the root cannot be read through a link inside it
- [ ] A path that does not exist yet still resolves, so `write` works
- [ ] The comment says what the check is worth and what it is not
