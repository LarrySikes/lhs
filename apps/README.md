# LHS sample apps

Apps you can **run** or **build into a standalone executable**.

## Day planner (interactive CLI)

```bash
cargo run -p npc --bin lhsc -- run apps/day_planner.lhs
./target/debug/lhsc build apps/day_planner.lhs -o day_planner
./day_planner
```

## Busy bench (numeric / parallel — two-language demo)

Hot loops + `task`/`await`. Good stand-in for “prototype then rewrite in C.”

```bash
cargo run -p npc --bin lhsc -- run apps/busy_bench.lhs
./target/debug/lhsc build apps/busy_bench.lhs -o busy_bench
./busy_bench
```

## Weather (HTTP + JSON — Python weather-app shape)

CLI port of the usual OpenWeatherMap example (no Flask/Tk). Needs `curl`
and `OPENWEATHER_API_KEY`.

```bash
export OPENWEATHER_API_KEY=your_key
CITY=Orlando ./target/debug/lhsc run apps/weather.lhs
./target/debug/lhsc build apps/weather.lhs -o weather
./weather London
```

Scan a Python tree first: `./target/debug/lhsc py-check path/to/py/project`  
Guide: `docs/PYTHON.md`.

## Mandelbrot (ASCII fractal — two-language demo)

Float-heavy numeric image. Same story: one language, then ship a binary.

```bash
cargo run -p npc --bin lhsc -- run apps/mandelbrot.lhs
./target/debug/lhsc build apps/mandelbrot.lhs -o mandelbrot
./mandelbrot
```

Data for the planner: `/tmp/lhs_planner.dat`, `/tmp/lhs_contacts.dat`.  
Non-interactive todo smoke only: `examples/14_day_planner.lhs`.
