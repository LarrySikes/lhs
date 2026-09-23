# busy_bench — numeric stress (two-language demo)
#
# Classic pitch: prototype a hot loop in a friendly language, then rewrite
# in C/Rust for speed or shipping. Here we keep one language and can also:
#   lhsc build apps/busy_bench.lhs -o busy_bench
#
# Run:
#   cargo run -p npc --bin lhsc -- run apps/busy_bench.lhs
#   cargo run -p npc --bin lhsc -- run --jit apps/busy_bench.lhs

# Shallow recursive work unit (keep depth modest — no while/for yet).
fn chunk(n: i32, acc: i32) -> i32 {
    if n <= 0 {
        return acc
    }
    # mix int + float so the JIT/native path does real numeric work
    let f: f64 = (n as f64) * 1.000001 + 0.5
    let bit = (f * f) as i32
    chunk(n - 1, acc + n + bit)
}

fn serial_rounds(r: i32, work: i32, acc: i32) -> i32 {
    if r <= 0 {
        return acc
    }
    serial_rounds(r - 1, work, acc + chunk(work, 0))
}

# Four parallel workers — same math, structured tasks (no shared mutation).
# Tasks do not capture outer locals; use literals (or pass via helpers).
fn parallel_batch() -> i32 {
    let a = task { chunk(60, 0) }
    let b = task { chunk(60, 0) }
    let c = task { chunk(60, 0) }
    let d = task { chunk(60, 0) }
    await a + await b + await c + await d
}

fn parallel_rounds(r: i32, acc: i32) -> i32 {
    if r <= 0 {
        return acc
    }
    parallel_rounds(r - 1, acc + parallel_batch())
}

fn main() {
    # Tuned for interpreter stack limits (~work 60, rounds 80).
    let work = 60
    let rounds = 80

    print("LHS busy_bench")
    print("work=" + work + "  serial_rounds=" + rounds)

    let t0 = now_ms()
    let s = serial_rounds(rounds, work, 0)
    let t1 = now_ms()
    print("serial checksum=" + s)
    print("serial ms=" + (t1 - t0))

    let pr = 20
    print("parallel_rounds=" + pr + " (4 tasks each)")
    let t2 = now_ms()
    let p = parallel_rounds(pr, 0)
    let t3 = now_ms()
    print("parallel checksum=" + p)
    print("parallel ms=" + (t3 - t2))

    print("ok - build standalone with:")
    print("  lhsc build apps/busy_bench.lhs -o busy_bench")
}
