---
id: 247
title: Give the site a terminal shell and a peculiar front desk
type: feature
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
stratum: '0'
area: site
proof: Generated pages and all checks pass; desktop/mobile shell, directory navigation and native disclosures work in a real browser
---

## Problem

## Proposal

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## 2026-09-15

A two-pane terminal frame separates the directory from the reading pane; the directory collapses initially on small screens but remains a native open disclosure without JavaScript. Real section links, active spec location, first-burial and manual entry points replace the old flat header. Whimsy is confined to the front desk and framing; technical prose and bitmap assets are unchanged. Rejected a fake command prompt and global shortcut interception: ordinary links, Tab and Enter already provide the useful behavior. Desktop/mobile screenshots reviewed; browser coverage includes lamp modes, keyboard disclosures and navigation.

## 2026-09-15

Linux CI passed every browser assertion but exposed a Chrome profile-cleanup race: a helper can finish a write after the parent exits. Temporary-profile removal now retries transient directory errors five times with a bounded delay and still fails if cleanup cannot complete.
