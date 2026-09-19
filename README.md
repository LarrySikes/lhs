# newproj (working title)

A small, memory-safe, native language aimed at the two-language problem:
easy to write, fast to run, safe by default — with tooling that humans and AI agents can both drive.

This is **not** a fork of IWBasic. Lessons from `iwbc` (ship a compiler + runtime + samples) apply; the language surface and semantics are new.

## Status

Bootstrap: manifesto, decisions, example programs, and a Rust compiler skeleton that is not yet a real frontend.

## Quick start (once the compiler exists)

```bash
cargo run -p npc -- check examples/01_hello.np
cargo run -p npc -- run examples/01_hello.np
cargo test
```

## Layout

| Path | Purpose |
|------|---------|
| `docs/MANIFESTO.md` | Goals and non-goals |
| `docs/DECISIONS.md` | Locked design choices |
| `examples/` | Spec-by-example programs |
| `crates/npc` | Compiler CLI (`npc`) |
| `crates/np_syntax` | Lexer / parser (stub) |
| `crates/np_hir` | Typed IR (stub) |

## Related

IWBasic / `iwbc` remains at `~/src/new0423/iwb_project` for desktop BASIC work.
