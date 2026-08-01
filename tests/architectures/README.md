# `tests/architectures/` — 35 declarations, and nothing runs them

**`pending-lava-test-runner:` these 35 `*.test.tlisp` files have no parser, no
runner, and no gate. They are inert. Do not read this directory as coverage.**

Measured 2026-08-01, with the denominators stated:

| probe | result |
|---|---|
| `*.test.tlisp` files here | **35** |
| references to `deflava-test` / `LavaTest` anywhere in `src/` | **0** |
| references to `tests/architectures` in `src/` or `Cargo.toml` | **0** |
| workflows consuming them (`security-gate`, `auto-release`, `pre-merge-gate`) | **0 of 3** |

## Why the generic runner does not pick them up

`pleme-io/actions/tlisp-test` discovers `<base>.test.tlisp` and runs it against a
sibling `<base>.tlisp` unit. These files have **no sibling unit** — and they are
not `(deftest …)`/`(assert …)` at all. They are a domain vocabulary:

```lisp
(deflava-test akeyless-aws-integration/default
  :architecture akeyless-aws-integration
  :bindings (:access-key-id "AKIA…" :secret-access-key "x")
  :assertions ((resource-exists akeyless-target-aws "prod-aws-target")
               (ref-valid)))
```

So wiring `tlisp-test` here would not help: it would find 35 files, fail to
locate a unit for each, and report 35 errors. The missing piece is a
**`deflava-test` interpreter** — something that resolves `:architecture`,
applies `:bindings`, renders, and evaluates each `:assertions` form against the
rendered output.

## Why this is worth a file rather than a silent TODO

This is the strongest form of the failure-that-looks-like-success shape. A gate
that passes vacuously at least runs; these never execute, while a directory of
35 named architecture tests reads to any human or agent as proof those
architectures are verified. Nothing is red, so nothing prompts a second look.

Found while closing the sibling defect in `pleme-io/actions`: `tlisp-test`
itself would log `all 0 file(s) passed` and exit 0 when discovery matched
nothing (fixed 2026-08-01 — it now fails below a `min-files` floor).

## When building the runner

- Assertion kinds present today: `resource-exists`, `ref-valid` (grep before
  assuming that is the full set — state the count you measured).
- An unknown assertion kind must be a **typed error**, never a silent pass — the
  same rule ensaio follows (`EnsaioError::UnknownAssertionKind`).
- Gate it in `pre-merge-gate.yml` with a floor on the number of tests executed,
  so a discovery regression cannot reproduce the defect this file documents.
