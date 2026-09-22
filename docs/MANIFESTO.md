# Manifesto — LHS

## Problem

Teams bounce between a high-productivity language (often Python) and a
high-performance / memory-safe language (Rust, C, C++). That split costs
rewrites, boundary bugs, and split tooling. LHS aims at one language that is
pleasant day-to-day and trustworthy in production.

## North star

- **Ease:** readable syntax, fast feedback, great errors.
- **Performance:** native code (C subset today; Cranelift/LLVM next); predictable costs.
- **Safety:** no unchecked null; memory rules that prevent use-after-free
  and data races by construction (exact model: see DECISIONS.md).
- **Concurrency:** structured tasks; parallelism is a first-class story,
  not an afterthought.
- **Agentic:** stable CLI, structured diagnostics, deterministic format
  and tests — so humans supervise agents instead of babysitting output.

## Non-goals (v1)

- Replacing the entire Python / Rust ecosystems in year one.
- JVM / .NET interop in v1 (C ABI first).
- Full Erlang-style distribution.
- A GUI framework or IWBasic CONTROL model.
- Forking or evolving IWBasic syntax.

## v1 product

A small language that:

1. Runs `examples/` via `lhsc run`; **`lhsc build` natives for the full language**
   (embed + `liblhs_rt`, 2026 v0.2). Optional `--emit=c` subset AOT.
2. Has `Option` / `Result`, pattern matching, and no implicit null.
3. Ships `lhsc check | run | test | fmt` with JSON diagnostics.
4. Has one concurrency demo (structured spawn + join).
5. Documents how AI agents should call the toolchain.

**v0.2 / v0.2.1 (2026)** completed the v1 product: full-language native packaging
(embed + `liblhs_rt`), Cranelift JIT/AOT including Option/Result/**custom ADTs**,
match exhaustiveness warnings, and agent docs.

**v0.3 (2026)** closes the main Cranelift gaps: **structs**, **receiver methods**,
**file I/O**. Remaining north-star work (GC, parallel tasks, Cranelift `extern`,
hot reload) is post-MVP.

## Relationship to iwbc

`iwbc` proves you can ship a compiler, runtime, samples, and Windows/Linux
packages. Reuse that *process*. Do not reuse BASIC semantics or the GTK
host as the language core.
