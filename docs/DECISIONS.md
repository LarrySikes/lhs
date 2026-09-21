# Design decisions (locked for MVP)

Update this file when a choice is deliberate. Prefer changing examples
before changing the compiler if the two disagree — examples are the spec.

## D1 — Implementation language

**Decision:** Rust for the compiler (`npc` and crates).  
**Why:** memory safety while building a memory-safe language; strong
ecosystem for parsers and CLIs.

## D2 — Memory model (MVP)

**Decision:** GC + explicit arenas for hot paths later; **not** a full
borrow checker in v1.  
**Why:** ship a coherent safe language faster; revisit ownership once
the frontend and tooling exist.  
**Constraint:** no raw unchecked pointers in user code; `unsafe` block
is a later, rare escape hatch.

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

**Decision (v0.2, 2026):** `lhsc build` embeds `.lhs` source and links
`liblhs_rt.a` so **every** program can become a native binary.  
`--emit=c` remains for a small AOT subset (no ADT/match/methods).  
**Next (optional):** Cranelift/LLVM true AOT without the interpreter runtime.  
**Why:** Completes the manifesto “ship runnable natives” goal without blocking
on a full lowering of ADTs.

## D6 — Interop

**Decision:** C ABI only in v1.  
**Why:** matches known shipping path from iwbc; JVM/.NET wait until the
language is worth embedding.

## D7 — Diagnostics

**Decision:** human text on stderr; `--json` for machine/agent use
(`file`, `span`, `code`, `message`, `help`).  
**Why:** agentic workflows need stable machine output from day one.

## D8 — Name

**Decision:** The language is **LHS**. Source extension is **`.lhs`**.
CLI binary is **`lhsc`** (LHS compiler). Repo directory may remain `newproj`.  
**Why:** Chosen 2026-09-20; replaces the deferred working title.
