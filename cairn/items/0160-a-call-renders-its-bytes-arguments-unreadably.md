---
id: 160
title: A call renders its bytes arguments unreadably
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-cli
stratum: '1'
proof: A rendered call can be read back as the call that was made
---

## The rendering

`lamp::said` quotes a `Str` argument and renders everything else through
`value`, which turns `Bytes` into its text with no delimiter:

```
8  call_foreign("dlopen", )   c1140919:1:42   pending
```

That is `call_foreign("dlopen", b"")`. An empty `Bytes` vanishes, and one
holding a comma or a parenthesis produces a line that cannot be read back as
the call it came from.

[§9.8](../spec/09-prelude.md) requires naming the symbol of a stratum-8 call in
`strata` output, which still happens — but the rest of the call is what tells a
reader whether it is the call they were looking for.

## Acceptance criteria

- [ ] `call_foreign("dlopen", b"")` renders as something that reads back
- [ ] Bytes holding a delimiter do not produce a line that lies
