# RELEASE — LHS v0.1.0

First usable toolchain for **LHS**.

## What's in

- `lhsc check` / `run` / `fmt` / `test` / `build`
- Lexer/parser for the example grammar (`.lhs`)
- Type checker with `--json` diagnostics
- Tree-walking interpreter (ADT, match, tasks, file I/O)
- Native `build` via C+`cc` for simple main/print programs

## Verify

```bash
cargo test
cargo run -p npc --bin lhsc -- test examples
cargo run -p npc --bin lhsc -- build examples/01_hello.lhs -o /tmp/lhs_hello && /tmp/lhs_hello
```

## Not yet

- Full Cranelift/LLVM AOT
- Package manager / crates.io publish
