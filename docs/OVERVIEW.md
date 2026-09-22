# LHS — system overview (v0.3, 2026)

LHS (“two-language problem” MVP) is a small statically typed language with a
Rust-hosted toolchain (`lhsc`). Source files use the `.lhs` extension. The
design goal is one language that is pleasant to write and can ship as native
code, with agent-friendly tooling.

## What you run

| Command | Role |
|---------|------|
| `lhsc check [--json]` | Parse + typecheck; JSON diagnostics for agents |
| `lhsc run` | Full-language tree-walking interpreter |
| `lhsc run --jit` | Cranelift in-process JIT (large subset) |
| `lhsc build` | Native binary: embeds source, links `liblhs_rt` (full language) |
| `lhsc build --emit=c` | Subset → C → `cc` (no ADT/methods) |
| `lhsc build --emit=cranelift` | Subset → object file → `cc` (true AOT, no interpreter) |
| `lhsc fmt` / `lhsc test` | Format; run all `examples/*.lhs` |

## Crate map

| Crate | Responsibility |
|-------|----------------|
| `np_syntax` | Lexer, parser, AST, formatter — **examples are the language spec** |
| `np_hir` | Name resolution, typechecker, match exhaustiveness warnings (`W0200`) |
| `np_eval` | Interpreter: values, ADTs, match, tasks, file I/O, stdlib |
| `np_codegen` | Embed driver + C subset emitter + link with `liblhs_rt` |
| `lhs_rt` | `lhs_run_source` C ABI — runs embedded source via interpreter |
| `lhs_jit` | Cranelift JIT + object AOT, bump-heap cells, host helpers |
| `npc` | CLI binary `lhsc` |

## Language surface (examples 01–14)

- Functions, locals, inference, `if` / `return`
- `Option` / `Result`, algebraic data types, `match`, `is`, `unwrap`
- Structs + methods (e.g. `Point.dist`)
- Strings: indexing, `.len()`, equality; chars and casts
- Structured `task` / `await` (sync today)
- `extern "C"` block shape; stdlib `print`, file I/O, `abs`/`min`/`max`/`assert`
- Sample app: day planner CLI (`examples/14_day_planner.lhs`, `apps/day_planner.lhs`)

## Execution backends

1. **Interpreter** — every feature; used by `run` and by default `build` (embed).
2. **C subset** — simple numeric/`print` programs only.
3. **Cranelift** — numeric + `f64` + strings + Option/Result + custom ADTs
   (≤2 fields) + structs/methods + match/`is`/`unwrap` + stdlib + file I/O +
   sync tasks. **Not** yet: `extern "C"` / unsafe FFI. Values use i64
   bit-patterns; floats via host `lhs_f*`; variants live on a **bump arena**
   (`docs/MEMORY.md`).

## Safety / memory / concurrency (honest status)

- No implicit null: `Option`/`Result` + exhaustiveness warnings.
- Memory model: Rust heap in the interpreter; bump arena for Cranelift cells.
  Full GC / arenas are next (DECISIONS D2), not a borrow checker in v1.
- Concurrency: structured tasks demo, still single-threaded — not parallel-first yet.

## Docs & verification

- Design: `docs/MANIFESTO.md`, `docs/DECISIONS.md`, `docs/MEMORY.md`, `docs/AGENT.md`
- Status (plain text): `docs/STATUS.txt`
- Releases: `docs/RELEASE_v0.1.md`, `docs/RELEASE_v0.2.md`, `docs/RELEASE_v0.3.md`
- CI: `.github/workflows/ci.yml` — build, unit tests, example suite, JIT/AOT smoke

```bash
cargo test --workspace && cargo run -p npc --bin lhsc -- test examples
```
