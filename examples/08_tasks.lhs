# 08 — structured concurrency (parallel)

fn work(id: i32) -> i32 {
    id * 2
}

fn main() {
    let a = task { work(1) }
    let b = task { work(2) }
    let sum = await a + await b
    print(sum)
}
