# RELEASE — v0.1.0

First usable toolchain for the newproj language (name TBD).

## What's in

- `npc check` / `run` / `fmt` / `test` / `build`
- Lexer/parser for the example grammar
- Type checker with `--json` diagnostics
- Tree-walking interpreter (ADT, match, tasks, file I/O)
- Native `build` via C+`cc` for simple main/print programs

## Verify

```bash
cargo test
cargo run -p npc -- test examples
cargo run -p npc -- build examples/01_hello.np -o /tmp/np_hello && /tmp/np_hello
```

## Not yet

- Full Cranelift/LLVM AOT
- Public language name
- Package manager / crates.io publish
