# Agent / automation notes

## Preferred commands

```bash
cargo run -p npc -- check examples/01_hello.np
cargo run -p npc -- check --json examples/10_bad_type.np
cargo test
```

## JSON diagnostics

`npc check --json` prints one JSON object per diagnostic:

```json
{"file":"...","span":[0,12],"code":"E0001","message":"...","help":"..."}
```

Agents should treat non-zero exit as failure and parse stdout for `--json`.

## Do not

- Invent APIs not shown in `examples/`.
- Port IWBasic CONTROL/WINDOW semantics into this language.
- Expand scope past `docs/MANIFESTO.md` v1 without updating DECISIONS.md.
