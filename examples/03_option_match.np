# 03 — option and match (no null)

fn find_digit(s: str, i: i32) -> Option<i32> {
    if i < 0 || i >= s.len() {
        return None
    }
    let c = s[i]
    if c >= '0' && c <= '9' {
        Some(c as i32 - '0' as i32)
    } else {
        None
    }
}

fn main() {
    match find_digit("a7", 1) {
        Some(d) => print(d),
        None => print("missing"),
    }
}
