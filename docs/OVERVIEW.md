# LHS — system overview (v0.5, 2026)

LHS (“two-language problem” MVP) is a small statically typed language with a
Rust-hosted toolchain (`lhsc`). Source files use the `.lhs` extension.

## What you run

| Command | Role |
|---------|------|
| `lhsc check [--json]` | Parse + typecheck; JSON diagnostics for agents |
| `lhsc run` | Full-language interpreter (parallel tasks) |
| `lhsc run --jit` | Cranelift in-process JIT |
| `lhsc watch [--jit]` | Re-run on file change (hot reload) |
| `lhsc build` | Native binary: embeds source, links `liblhs_rt` |
| `lhsc build --emit=c` | Subset → C → `cc` |
| `lhsc build --emit=cranelift` | True AOT via Cranelift object + stubs |
| `lhsc fmt` / `lhsc test` | Format; run `examples/*.lhs` |

## Crate map

| Crate | Responsibility |
|-------|----------------|
| `np_syntax` | Lexer, parser, AST, formatter |
| `np_hir` | Typechecker, exhaustiveness (`W0200`) |
| `np_eval` | Interpreter + parallel `task`/`await` + stdlib |
| `np_codegen` | Embed driver + C subset emitter |
| `lhs_mem` | Shared RC heap + bump scratch |
| `lhs_rt` | `lhs_run_source` embed runtime |
| `lhs_jit` | Cranelift JIT + AOT |
| `npc` | CLI `lhsc` |

## Docs

`docs/STATUS.txt`, `docs/PACKAGES.md`, `docs/MANIFESTO.md`, `docs/DECISIONS.md`,
`docs/MEMORY.md`, `docs/AGENT.md`, `docs/RELEASE_v0.5.md`
