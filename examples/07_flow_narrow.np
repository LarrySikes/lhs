# 07 — flow-sensitive narrowing

fn label(x: Option<i32>) -> str {
    if x is None {
        return "none"
    }
    // after the guard, x is treated as Some
    let n = x.unwrap()
    if n > 0 {
        "pos"
    } else {
        "nonpos"
    }
}

fn main() {
    print(label(Some(3)))
    print(label(None))
}
