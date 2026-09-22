# Memory model notes (LHS)

## Today (v0.4)

| Path | Allocation |
|------|------------|
| Interpreter (`lhsc run`) | Rust heap via `Value` |
| Embed (`lhsc build`) | Same, inside `liblhs_rt` (calls `lhs_mem::reset` per run) |
| Cranelift JIT | **Shared `lhs_mem` bump arena** for cells + dynamic strings |
| Cranelift AOT | Matching C bump heap in generated stubs (same cell layout) |

User code has no raw pointers (except explicit `extern` / `unsafe`). Cells are
tagged `(tag, payload, extra)` on the bump heap. Parallel tasks do not share
mutable LHS values across threads (message-style: compute then `await`).

## Next (optional hardening)

1. Mark-sweep or RC when values outlive a single `main` invocation.
2. Optional explicit arenas for hot loops (DECISIONS D2).
3. Unify AOT stubs to link `liblhs_mem.a` instead of duplicated C bump.
