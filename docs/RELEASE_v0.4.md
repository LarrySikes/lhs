# RELEASE — LHS v0.4 (2026) · north-star complete

## Highlights

- **Shared bump arena** crate `lhs_mem` (JIT + `lhs_rt` reset)
- **Parallel `task` / `await`** — OS threads in interpreter and Cranelift (spawn/join)
- **Cranelift `extern "C"`** — e.g. `puts` + `c"..."` (example 09 JIT/AOT)
- **`lhsc watch [--jit] <file>`** — hot reload on save for agent/human loops
- Example `15_parallel_busy.lhs`

## Verify

```bash
cargo test --workspace
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- run --jit examples/09_c_abi.lhs
cargo run -p npc --bin lhsc -- run --jit examples/08_tasks.lhs
cargo run -p npc --bin lhsc -- build examples/09_c_abi.lhs -o /tmp/a --emit=cranelift && /tmp/a
```

See `docs/STATUS.txt`.
