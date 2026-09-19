# 04 — result and error propagation

fn parse_i32(s: str) -> Result<i32, str> {
    // v1: library will provide real parse; example shows shape only
    if s == "7" {
        Ok(7)
    } else {
        Err("bad int")
    }
}

fn main() {
    match parse_i32("7") {
        Ok(n) => print(n),
        Err(e) => print(e),
    }
}
