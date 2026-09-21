# Agent / automation notes — LHS (v0.2, 2026)

## Commands

```bash
cargo run -p npc --bin lhsc -- check [--json] <file.lhs>
cargo run -p npc --bin lhsc -- run <file.lhs>
cargo run -p npc --bin lhsc -- fmt [--write] <file.lhs>
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out>
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out> --emit=c   # subset AOT
```

Build `lhs_rt` first if needed: `cargo build -p lhs_rt`.

## JSON diagnostics

`lhsc check --json` → one JSON object per diagnostic on stdout. Non-zero exit = failure.
