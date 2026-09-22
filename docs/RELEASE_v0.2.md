# RELEASE — LHS v0.2.1 (2026) · complete

## Highlights

- Full-language **embed** native binaries (`liblhs_rt`)
- **Cranelift** JIT + `--emit=cranelift` AOT: Option/Result, **custom ADTs**,
  `f64`, strings, match/`is`/`unwrap`, stdlib, sync tasks
- Match exhaustiveness warnings · bump arena · CI · licenses
- Still embed/interpret for: receiver methods, extern, file I/O

## Verify

```bash
cargo test --workspace
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- run --jit examples/05_adt.lhs
cargo run -p npc --bin lhsc -- build examples/05_adt.lhs -o /tmp/a --emit=cranelift && /tmp/a
```

See `docs/OVERVIEW.md`.
