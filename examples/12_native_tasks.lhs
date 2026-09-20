# 12 — native-build friendly helpers

fn double(n: i32) -> i32 {
    n * 2
}

fn main() {
    let a = task { double(3) }
    let b = task { double(4) }
    let sum = await a + await b
    print(sum)
}
