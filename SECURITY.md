# Security

## Supported versions

None yet. Nothing is implemented and nothing is released, so there is no
supported version and no version to patch. This file describes what will happen
when there is.

## Reporting a vulnerability

Report privately through GitHub Security Advisories:

**<https://github.com/oddurs/nether-c/security/advisories/new>**

Do not open a public issue for a vulnerability.

Expect an acknowledgement within seven days and an assessment within thirty. If
a report goes unanswered for two weeks, escalate by opening a public issue that
says only that you are waiting on a private report — no details.

## What counts

Once there is an implementation, the interesting attack surface is narrow and
worth naming in advance:

- **The decoder.** `nether-ledger` parses bytes from a store that may be shared
  between people who do not trust each other. Memory unsafety, panics, and
  resource exhaustion in the decoder are all in scope. The workspace forbids
  `unsafe`, which narrows this but does not close it.
- **Non-canonical acceptance.** A decoder that accepts two different byte
  strings as the same value breaks every reproducibility claim in the
  specification. Treat it as a vulnerability, not a bug.
- **Capability escape.** Any way to reach a stratum without a `descend` that
  granted it, or to lower a value's depth other than through `seal` and
  `shade`, is a vulnerability in the core promise of the language.
- **Witness omission.** A path where a world-derived value reaches the program
  before its witness is written to the ledger.

What is not in scope: a program you wrote reaching stratum 8 and doing something
unpleasant. That is what stratum 8 is, it is marked in the trace, and
[§1.7](spec/01-strata.md) says so.
