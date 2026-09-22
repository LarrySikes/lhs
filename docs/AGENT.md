# Agent / automation notes — LHS (v0.4 complete, 2026)

## Commands

```bash
cargo run -p npc --bin lhsc -- check [--json] <file.lhs>
cargo run -p npc --bin lhsc -- run [--jit] <file.lhs>
cargo run -p npc --bin lhsc -- watch [--jit] <file.lhs>
cargo run -p npc --bin lhsc -- fmt [--write] <file.lhs>
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out>
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out> --emit=cranelift
```

### Backend choice

| Goal | Flag |
|------|------|
| Full language binary | default `build` (embed) |
| Fast AOT | `--emit=cranelift` |
| Fast feedback | `run --jit` or `watch --jit` |
| Interpret | `run` |

Cranelift: stdlib, ADTs, structs/methods, file I/O, `extern "C"`, parallel tasks.

## JSON diagnostics

`lhsc check --json` → one object per diagnostic. Errors fail the process;
warnings alone exit 0.

## Hot reload

`lhsc watch file.lhs` polls mtime and re-runs (use while editing with an agent).
