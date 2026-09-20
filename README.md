# newproj (working title)

A small, memory-safe, native language aimed at the two-language problem:
easy to write, fast to run, safe by default — with tooling that humans and AI agents can both drive.

**Compiler binary name: `npc`** (newproj compiler). The *language* name is still TBD.

This is **not** a fork of IWBasic. Lessons from `iwbc` (ship a compiler + runtime + samples) apply; the language surface and semantics are new.

## Status

Lexer + parser for `fn` / `let` / calls / literals. `npc check` parses `.np` files and reports syntax (and a few stub type) errors.

## Quick start (once the compiler exists)

```bash
cargo run -p npc -- check examples/01_hello.np
cargo run -p npc -- run examples/01_hello.np
cargo test
```

## Compiler source

| Path | Purpose |
|------|---------|
| `crates/npc/src/main.rs` | CLI entry (`npc check`) |
| `crates/np_syntax/src/lib.rs` | Lexer, parser, AST |
| `crates/np_hir/src/lib.rs` | Early checks / typed IR |

Absolute path on this machine: `/home/iwlnx/src/newproj/crates/`

## Layout

| Path | Purpose |
|------|---------|
| `docs/MANIFESTO.md` | Goals and non-goals |
| `docs/DECISIONS.md` | Locked design choices |
| `examples/` | Spec-by-example programs |
| `crates/npc` | Compiler CLI (`npc`) |
| `crates/np_syntax` | Lexer / parser / AST |
| `crates/np_hir` | Typed IR (early stub) |

## Related

IWBasic / `iwbc` remains at `~/src/new0423/iwb_project` for desktop BASIC work.
