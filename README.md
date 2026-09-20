# LHS — v0.1

**LHS** is a small, memory-safe-oriented language with a Rust-hosted toolchain.

**Compiler:** `lhsc` (LHS compiler).  
**Source files:** `.lhs`

## Commands

```bash
cargo run -p npc --bin lhsc -- check examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run examples/01_hello.lhs
cargo run -p npc --bin lhsc -- fmt examples/01_hello.lhs
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/01_hello.lhs -o /tmp/lhs_hello
```

(After `cargo install --path crates/npc`, just use `lhsc`.)

## Compiler source

| Path | Role |
|------|------|
| `crates/npc` | CLI (`lhsc`) |
| `crates/np_syntax` | Lexer, parser, AST, formatter |
| `crates/np_hir` | Type checker |
| `crates/np_eval` | Interpreter (`run`) |
| `crates/np_codegen` | C subset emitter (`build`) |

Repo path: `/home/iwlnx/src/newproj/`

## Status

- [x] Parse `examples/`
- [x] Typecheck (JSON diagnostics)
- [x] `lhsc run` interpreter
- [x] `lhsc fmt` / `lhsc test`
- [x] `lhsc build` → native binary for simple programs
- [ ] Cranelift backend for full language
