# LHS — v0.4 (2026) · complete

**LHS** is a small, memory-safe-oriented language with a Rust-hosted toolchain.

**Compiler:** `lhsc` · **Sources:** `.lhs` · **License:** MIT OR Apache-2.0

One-page tour: [`docs/OVERVIEW.md`](docs/OVERVIEW.md) · Status: [`docs/STATUS.txt`](docs/STATUS.txt)

## Commands

```bash
cargo run -p npc --bin lhsc -- check examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run examples/01_hello.lhs
cargo run -p npc --bin lhsc -- run --jit examples/09_c_abi.lhs
cargo run -p npc --bin lhsc -- watch examples/01_hello.lhs
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/06_struct_method.lhs -o /tmp/lhs_06
cargo run -p npc --bin lhsc -- build examples/09_c_abi.lhs -o /tmp/a --emit=cranelift
```

## Status

- [x] Parse / typecheck / fmt / test / JSON diagnostics
- [x] Full-language `run` + embed `build` (`liblhs_rt`)
- [x] Cranelift JIT/AOT: ADTs, structs/methods, file I/O, `extern "C"`, parallel tasks
- [x] Shared `lhs_mem` bump · `lhsc watch` hot reload
- [x] CI + licenses

See `docs/OVERVIEW.md` and `docs/STATUS.txt`.
