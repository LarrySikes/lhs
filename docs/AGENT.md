# Agent / automation notes (v0.1)

## Preferred commands

```bash
cargo run -p npc -- check [--json] <file.np>
cargo run -p npc -- run <file.np>
cargo run -p npc -- fmt [--write] <file.np>
cargo run -p npc -- test examples
cargo run -p npc -- build <file.np> -o <out>   # simple subset only
```

## JSON diagnostics

`npc check --json` prints one JSON object per diagnostic on stdout:

```json
{"file":"...","span":[0,12],"code":"E0001","message":"...","help":"..."}
```

Non-zero exit = failure. Agents should prefer `--json` for machine parsing.

## Do not

- Invent APIs not shown in `examples/` or `docs/`.
- Port IWBasic CONTROL/WINDOW semantics into this language.
- Assume `npc build` supports ADT/match/async — use `npc run` for those.
