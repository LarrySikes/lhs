# RELEASE — LHS v0.6 (2026) · modules

## Highlights

- **`use` imports** — `use math` loads `stdlib/math.lhs` (nested paths OK)
- **`lhsc lib`** — lists builtins and stdlib modules
- **`$LHS_PATH`** search + embed builds merge imports into the binary
- Example `17_use_math.lhs`; stdlib `math` + `io`

## Verify

```bash
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- lib
cargo run -p npc --bin lhsc -- run --jit examples/17_use_math.lhs
```
