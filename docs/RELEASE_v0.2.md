# RELEASE — LHS v0.2.0 (2026)

## Highlights

- **`lhsc build`** produces a native binary for the **full** language by
  embedding source and linking `liblhs_rt` (interpreter runtime).
- **`--emit=c`** keeps the subset C translator for simple programs.
- Stdlib helpers: `abs`, `min`, `max`, `assert` (+ existing print / file I/O).
- Example suite: 13 programs; `lhsc test examples` all green.

## Verify (2026)

```bash
cargo build -p lhs_rt -p npc --bin lhsc
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/06_struct_method.lhs -o /tmp/lhs_06
/tmp/lhs_06   # prints 5
```

## Layout

- Language name: **LHS**
- Extension: **`.lhs`**
- CLI: **`lhsc`**
- C backend / packaging: `crates/np_codegen`
- Runtime staticlib: `crates/lhs_rt`
