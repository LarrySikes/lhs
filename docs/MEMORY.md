# Memory model notes (LHS)

## Today (v0.5)

| Path | Allocation |
|------|------------|
| Interpreter (`lhsc run`) | Rust heap via `Value` |
| Embed (`lhsc build`) | Same; `lhs_rt` calls `lhs_mem::reset` per run |
| Cranelift JIT | **`lhs_mem` RC cells/strings** + bump scratch |
| Cranelift AOT | Matching malloc-RC cells in stubs |

`lhs_retain` / `lhs_release` / `lhs_gc` are the stable C ABI. User code has no
raw pointers except explicit `extern` / `unsafe`. Parallel tasks do not share
mutable LHS values across threads.

## Optional later

1. Compacting mark-sweep for long sessions.
2. Link AOT binaries directly against `liblhs_mem.a` (stubs already RC-compatible).
3. Explicit arenas for hot loops (DECISIONS D2).
