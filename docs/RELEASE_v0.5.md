# RELEASE — LHS v0.5 (2026) · hardening complete

## Highlights

- **RC heap** in `lhs_mem` (`retain`/`release`/`gc`); cells and dynamic strings
  are refcounted (AOT stubs match)
- **Extended stdlib:** `getenv`, `argc`/`arg`, `exit`, `sleep_ms`, `now_ms`,
  `eprint`, `gc`
- **`docs/PACKAGES.md`** + `stdlib/` reserved for future modules
- Example `16_stdlib_ext.lhs`

## Verify

```bash
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- run --jit examples/16_stdlib_ext.lhs
```
