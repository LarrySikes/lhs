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

## Mandelbrot (ASCII fractal — two-language demo)

Float-heavy numeric image. Same story: one language, then ship a binary.

```bash
cargo run -p npc --bin lhsc -- run apps/mandelbrot.lhs
./target/debug/lhsc build apps/mandelbrot.lhs -o mandelbrot
./mandelbrot
```

Data for the planner: `/tmp/lhs_planner.dat`, `/tmp/lhs_contacts.dat`.  
Non-interactive todo smoke only: `examples/14_day_planner.lhs`.
