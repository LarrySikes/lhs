# 09 — C ABI interop (shape)

extern "C" {
    fn puts(s: *const u8) -> i32
}

fn main() {
    // v1 will provide safer string bridging; example locks the intent
    unsafe {
        puts(c"hello from C\0")
    }
}
