# RELEASE — LHS v0.3 (2026) · complete

## Highlights

- Cranelift JIT/AOT now covers **structs**, **receiver methods** (e.g. `Point.dist`),
  **`.sqrt()`**, and **`read_file` / `write_file`**
- Only remaining Cranelift gap vs embed: **`extern "C"` / unsafe FFI** (example 09)
- Full-language path unchanged: default `lhsc build` (embed) + `lhsc run`

## Verify

```bash
cargo test --workspace
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- run --jit examples/06_struct_method.lhs
cargo run -p npc --bin lhsc -- run --jit examples/11_file_io.lhs
cargo run -p npc --bin lhsc -- build examples/06_struct_method.lhs -o /tmp/a --emit=cranelift && /tmp/a
```

See `docs/OVERVIEW.md` and `docs/STATUS.txt`.
