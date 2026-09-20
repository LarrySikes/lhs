# newproj (working title) — v0.1

A small, memory-safe-oriented language with a Rust-hosted toolchain (`npc`).

**Compiler binary: `npc`.** Language name still TBD.

## Commands

```bash
cargo run -p npc -- check examples/01_hello.np
cargo run -p npc -- run examples/01_hello.np
cargo run -p npc -- fmt examples/01_hello.np
cargo run -p npc -- test examples
cargo run -p npc -- build examples/01_hello.np -o /tmp/np_hello
```

## Compiler source

| Path | Role |
|------|------|
| `crates/npc` | CLI |
| `crates/np_syntax` | Lexer, parser, AST, formatter |
| `crates/np_hir` | Type checker |
| `crates/np_eval` | Interpreter (`run`) |
| `crates/np_codegen` | C subset emitter (`build`) |

Absolute path: `/home/iwlnx/src/newproj/crates/`

## v0.1 status

- [x] Parse `examples/`
- [x] Typecheck (JSON diagnostics)
- [x] `npc run` interpreter (Option/Result/match/tasks/file I/O)
- [x] `npc fmt` / `npc test`
- [x] `npc build` → native binary for simple main/print programs
- [ ] Cranelift backend for full language (see DECISIONS D5)
