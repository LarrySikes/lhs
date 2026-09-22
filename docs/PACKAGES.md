# Packages / stdlib (LHS v0.5)

LHS does not yet have a package registry. Built-in stdlib lives in the
compiler/runtime:

## Builtins

| Name | Role |
|------|------|
| `print` / `eprint` | stdout / stderr |
| `read_file` / `write_file` | Result-based file I/O |
| `read_line` / `str_slice` / `find_char` | string helpers (interpret/embed) |
| `abs` / `min` / `max` / `assert` | numeric / assert |
| `getenv` / `argc` / `arg` | process environment |
| `exit` / `sleep_ms` / `now_ms` | process control / time |
| `gc` | RC heap stats (frees so far) |

## Layout for future packages

```
stdlib/          # reserved for .lhs library modules
apps/            # sample applications
examples/        # language spec + demos
```

Import syntax (`use foo`) is deferred; for now call builtins directly.
Agents should prefer `lhsc check --json` and `lhsc watch`.
