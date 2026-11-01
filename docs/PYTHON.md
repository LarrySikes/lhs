Author: Larry Sikes
Last modified: 2026-11-01

# LHS and Python projects

## Where the source is

| What | Path |
|------|------|
| LHS repo (local) | `/home/iwlnx/src/newproj` |
| GitHub | https://github.com/LarrySikes/lhs |
| Compiler CLI | `crates/npc` → binary `lhsc` |
| Language crates | `crates/np_syntax`, `np_hir`, `np_eval`, `np_codegen`, `lhs_*` |
| Apps / examples | `apps/`, `examples/` |
| This guide | `docs/PYTHON.md` |

## Goal

For LHS to be truly useful, it should be able to handle most existing Python
projects. That bar is **not fully met** yet.

## Roadmap (Python-scale usefulness)

| Step | Status | What |
|------|--------|------|
| 1 | **Done** | `http_get`, `json_get`, `lhsc py-check`, `apps/weather.lhs` |
| 2 | **Done** | `http_post`, `http_request` (method + headers + body), `re_find`, `re_replace` |
| 3 | **Done** | SQLite bridge: `db_exec` / `db_query` (needs `sqlite3` on PATH) |
| 4 | Next | Loops (`while` / `for`) — big help for ports; recursion only today |
| 5 | Later | Minimal HTTP **server** (or keep web UIs in Python) |
| 6 | Later | GUI — not a v1 goal; keep Tk/Qt in Python |
| 7 | Later | Automatic Python → LHS translator for a safe subset |

## What works today

| Need in many Python apps | LHS today |
|--------------------------|-----------|
| HTTP GET | `http_get(url) -> Result` (curl) |
| HTTP POST (JSON body) | `http_post(url, body) -> Result` |
| HTTP + headers | `http_request(method, url, headers, body)` |
| JSON field | `json_get(doc, "a.b.0.c") -> Option` |
| Regex | `re_find` / `re_replace` |
| SQLite | `db_exec(path, sql)` / `db_query(path, sql)` → JSON |
| Files | `read_file` / `write_file` |
| Env / argv | `getenv`, `argc`, `arg` |
| Scan a Python project | `lhsc py-check <dir\|file.py>` |

```bash
cd /home/iwlnx/src/newproj
cargo build -p npc --bin lhsc

export OPENWEATHER_API_KEY=your_key
CITY=Orlando ./target/debug/lhsc run apps/weather.lhs

./target/debug/lhsc run examples/20_json_get.lhs
./target/debug/lhsc run examples/21_http_regex.lhs
./target/debug/lhsc run examples/22_sqlite.lhs
./target/debug/lhsc py-check ~/Downloads/pywork/weather-app-master
```

`http_request` headers are newline-separated lines like `Authorization: Bearer …`.

## Check a Python project

```bash
./target/debug/lhsc py-check ~/Downloads/pywork/weather-app-master
```

Exit codes: `0` portable-ish · `1` bad path · `2` hard gaps (Flask, Django, Tk, …)

## Still a gap

- Web frameworks and desktop GUIs  
- Sessions / cookies / multipart uploads (use raw `http_request` for simple cases)  
- Postgres / ORMs, numpy/pandas, PyPI installs  
- Automatic whole-project translation  
- `while` / `for` loops  

## Honest path

1. `lhsc py-check` on the Python tree.  
2. Port network + JSON + regex with the builtins above.  
3. Keep Flask/Tk in Python for now, or rewrite the core as a CLI.  
4. `lhsc build` when you want a standalone binary.
