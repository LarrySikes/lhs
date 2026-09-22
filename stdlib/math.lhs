# math — small numeric helpers (stdlib)

fn square(n: i32) -> i32 {
    n * n
}

fn clamp(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else {
        if x > hi {
            hi
        } else {
            x
        }
    }
}

fn abs_i(n: i32) -> i32 {
    if n < 0 {
        0 - n
    } else {
        n
    }
}
