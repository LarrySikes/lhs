# LHS — v0.5 (2026) · complete

**LHS** is a small, memory-safe-oriented language with a Rust-hosted toolchain.

**Compiler:** `lhsc` · **Sources:** `.lhs` · **License:** MIT OR Apache-2.0

[`docs/OVERVIEW.md`](docs/OVERVIEW.md) · [`docs/STATUS.txt`](docs/STATUS.txt)

## Commands

```bash
cargo run -p npc --bin lhsc -- check examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run --jit examples/16_stdlib_ext.lhs
cargo run -p npc --bin lhsc -- watch examples/01_hello.lhs
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/09_c_abi.lhs -o /tmp/a --emit=cranelift
```

## Status

- [x] Toolchain + embed + Cranelift JIT/AOT
- [x] Parallel tasks, extern "C", structs/methods, file I/O
- [x] RC heap (`lhs_mem`) + extended stdlib + `lhsc watch`
- [x] CI + licenses

See `docs/STATUS.txt`.
