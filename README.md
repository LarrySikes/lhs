# LHS — v0.2.1 (2026) · complete

**LHS** is a small, memory-safe-oriented language with a Rust-hosted toolchain.

**Compiler:** `lhsc` · **Sources:** `.lhs` · **License:** MIT OR Apache-2.0

One-page tour: [`docs/OVERVIEW.md`](docs/OVERVIEW.md)

## Commands

```bash
cargo run -p npc --bin lhsc -- check examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run --jit examples/05_adt.lhs
cargo run -p npc --bin lhsc -- fmt examples/01_hello.lhs
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/06_struct_method.lhs -o /tmp/lhs_06
cargo run -p npc --bin lhsc -- build examples/05_adt.lhs -o /tmp/a --emit=cranelift
```

## Status

- [x] Parse / typecheck / fmt / test / JSON diagnostics
- [x] Full-language `run` + embed `build` (`liblhs_rt`)
- [x] `--emit=c` subset · `--emit=cranelift` / `run --jit` (ADT/Option/Result/f64/strings)
- [x] Match exhaustiveness warnings · bump arena (`docs/MEMORY.md`)
- [x] CI + licenses

Cranelift still skips **receiver methods**, **extern**, and **file I/O** (use
default `build` / `run`). See `docs/OVERVIEW.md`.
