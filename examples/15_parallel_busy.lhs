# 15 — parallel tasks (same pattern as 08; runs on threads)

fn work(n: i32) -> i32 {
    n * n
}

fn main() {
    let a = task { work(3) }
    let b = task { work(4) }
    let c = task { work(5) }
    print(await a + await b + await c)
}
