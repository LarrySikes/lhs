# LHS — v0.2 (2026)

**LHS** is a small, memory-safe-oriented language with a Rust-hosted toolchain.

**Compiler:** `lhsc` · **Sources:** `.lhs` · **Year:** 2026

## Commands

```bash
cargo run -p npc --bin lhsc -- check examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run examples/01_hello.lhs
cargo run -p npc --bin lhsc -- fmt examples/01_hello.lhs
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/03_option_match.lhs -o /tmp/lhs_03
# optional subset AOT:
cargo run -p npc --bin lhsc -- build examples/01_hello.lhs -o /tmp/h --emit=c
```

## Compiler source

| Path | Role |
|------|------|
| `crates/npc` | CLI (`lhsc`) |
| `crates/np_syntax` | Lexer, parser, AST, formatter |
| `crates/np_hir` | Type checker |
| `crates/np_eval` | Interpreter |
| `crates/np_codegen` | Native packaging + subset C emitter |
| `crates/lhs_rt` | `liblhs_rt.a` for `lhsc build` |

## Status (v0.2)

- [x] Parse / typecheck / fmt / test
- [x] `lhsc run` full examples
- [x] `lhsc build` native binaries for **all** language features (embed + `liblhs_rt`)
- [x] `--emit=c` subset translator
- [x] Stdlib: print, file I/O, abs/min/max/assert
- [ ] Cranelift AOT (optional future)
