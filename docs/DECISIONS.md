# Design decisions (locked for MVP)

Update this file when a choice is deliberate. Prefer changing examples
before changing the compiler if the two disagree — examples are the spec.

## D1 — Implementation language

**Decision:** Rust for the compiler (`npc` and crates).  
**Why:** memory safety while building a memory-safe language; strong
ecosystem for parsers and CLIs.

## D2 — Memory model (MVP)

**Decision:** Start with a **bump arena** for Cranelift-backed
`Option`/`Result` cells (`lhs_make_*` in `lhs_jit`); full GC + optional
explicit arenas later. **Not** a borrow checker in v1.  
**Why:** ship a coherent safe language faster; revisit ownership once
the frontend and tooling exist.  
**Constraint:** no raw unchecked pointers in user code; `unsafe` block
is a later, rare escape hatch.  
**Status (2026):** bump heap lives in the Cranelift host/stubs; interpreter
still uses Rust `Value` heap. Next: share one allocator with `lhs_rt`.

## D3 — Syntax family

**Decision:** brace-delimited, expression-oriented, significant types
with local inference (closer to Rust/Swift than indentation-Python).  
**Why:** unambiguous for agents and diffs; familiar to systems
programmers; indentation languages fight copy/paste and tooling.

## D4 — Concurrency (MVP)

**Decision:** structured `task` / `await` with a single-threaded async
runtime first; data-race freedom via no shared mutable aliasing across
tasks (message or immutable share).  
**Why:** explicit control without requiring a full actor runtime day one.

## D5 — Backend

**Decision (v0.2–v0.3, 2026):** three native paths:

1. **Default `lhsc build`** — embed `.lhs` + link `liblhs_rt.a` (full language).
2. **`--emit=c`** — subset C translator (no ADT/match/methods).
3. **`--emit=cranelift` / `run --jit`** — Cranelift object AOT and in-process
   JIT for numeric/`f64`/`print`/stdlib/`task`, Option/Result, **custom ADTs**
   (≤2 fields), **structs + receiver methods**, `match`/`is`/`unwrap`, string
   len/index/eq, **file I/O**. Still not: `extern "C"` / unsafe FFI.

**Post-v0.3:** shared GC with `lhs_rt`; real `extern` linking in Cranelift;
parallel tasks.  
**Why:** Completes the manifesto “ship runnable natives” goal without blocking
on every language feature in the optimizing path.

## D6 — Interop

**Decision:** C ABI only in v1.  
**Why:** matches known shipping path from iwbc; JVM/.NET wait until the
language is worth embedding.

## D7 — Diagnostics

**Decision:** human text on stderr; `--json` for machine/agent use
(`file`, `span`, `code`, `message`, `help`, `warning`).  
Warnings (`W*`, e.g. non-exhaustive match `W0200`) print but do not fail
`lhsc check` / `run`. Errors (`E*`) do.  
**Why:** agentic workflows need stable machine output from day one.

## D8 — Name

**Decision:** The language is **LHS**. Source extension is **`.lhs`**.
CLI binary is **`lhsc`** (LHS compiler). Repo directory may remain `newproj`.  
**Why:** Chosen 2026-09-20; replaces the deferred working title.
