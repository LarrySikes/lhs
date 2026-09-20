# Agent / automation notes — LHS

## Preferred commands

```bash
cargo run -p npc --bin lhsc -- check [--json] <file.lhs>
cargo run -p npc --bin lhsc -- run <file.lhs>
cargo run -p npc --bin lhsc -- fmt [--write] <file.lhs>
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build <file.lhs> -o <out>   # simple subset only
```

## JSON diagnostics

`lhsc check --json` prints one JSON object per diagnostic on stdout:

```json
{"file":"...","span":[0,12],"code":"E0001","message":"...","help":"..."}
```

Non-zero exit = failure. Agents should prefer `--json` for machine parsing.

## Do not

- Invent APIs not shown in `examples/` or `docs/`.
- Port IWBasic CONTROL/WINDOW semantics into LHS.
- Assume `lhsc build` supports ADT/match/async — use `lhsc run` for those.
