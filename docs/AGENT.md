# Agent / automation notes — LHS (v0.3 complete, 2026)

## Commands

```bash
cargo run -p npc --bin lhsc -- check [--json] <file.lhs>
cargo run -p npc --bin lhsc -- run [--jit] <file.lhs>
cargo run -p npc --bin lhsc -- fmt [--write] <file.lhs>
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out>
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out> --emit=c
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out> --emit=cranelift
```

Build backends if needed: `cargo build -p lhs_rt -p lhs_jit`.

### Backend choice

| Goal | Flag |
|------|------|
| Full language binary | default `build` (embed) |
| Fast subset AOT | `--emit=cranelift` or `--emit=c` |
| Fast feedback subset | `run --jit` |
| Full language interpret | `run` (no flags) |

Cranelift subset: `print`, `abs`/`min`/`max`/`assert`, numeric/`if`/locals,
user fns + **receiver methods**, sync `task`/`await`, Option/Result/custom ADTs,
structs (≤2 fields), string `len`/`[]`/`==`, `is`/`unwrap`/`.sqrt()`,
**`read_file`/`write_file`**. Not: `extern "C"` / unsafe FFI.

## JSON diagnostics

`lhsc check --json` → one object per diagnostic
(`file`, `span`, `code`, `message`, `help`, `warning`). Exit non-zero only for
errors; warnings alone exit 0.
