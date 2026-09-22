# Memory model notes (LHS)

## Today

| Path | Allocation |
|------|------------|
| Interpreter (`lhsc run`) | Rust heap via `Value` |
| Embed (`lhsc build`) | Same, inside `liblhs_rt` |
| Cranelift JIT/AOT | **Bump arena** for `Option`/`Result` cells (`lhs_make_none/some/ok/err`); string literals in rodata |

User code has no raw pointers. Cells are tagged `(tag, payload)` pairs on the bump heap.

## Next

1. Share one bump/GC between `lhs_rt` and Cranelift hosts.
2. Mark-sweep or RC when values outlive a single `main` invocation.
3. Optional explicit arenas for hot loops (DECISIONS D2).

Dynamic strings from `read_file` (and error messages) also live on the Cranelift
bump heap today.
