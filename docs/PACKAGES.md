# Packages / stdlib (LHS v0.6)

## Imports

```lhs
use math          # loads stdlib/math.lhs
use io            # loads stdlib/io.lhs
use std.io        # loads stdlib/std/io.lhs (nested)
use "math"        # same as use math
```

Search order: `$LHS_PATH` (colon-separated), `stdlib/` at repo root, the
source file's directory, then the repo root.

Imported modules are merged into one program (flat namespace). Nested `use`
inside a module is resolved recursively.

## Commands

```bash
lhsc lib                 # list builtins + stdlib modules
lhsc run examples/17_use_math.lhs
```

## Builtins

| Name | Role |
|------|------|
| `print` / `eprint` | stdout / stderr |
| `read_file` / `write_file` | Result-based file I/O |
| `read_line` / `str_slice` / `find_char` | string helpers |
| `abs` / `min` / `max` / `assert` | numeric / assert |
| `getenv` / `argc` / `arg` | process environment |
| `exit` / `sleep_ms` / `now_ms` | process control / time |
| `gc` | RC heap free-count |

## Layout

```
stdlib/          # .lhs library modules
apps/            # sample applications
examples/        # language spec + demos
```

There is no remote package registry yet; ship libraries as `.lhs` files on disk.
