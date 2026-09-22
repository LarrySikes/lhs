# 16 — extended stdlib (env, time, gc)

fn main() {
    print(argc())
    match arg(0) {
        Some(s) => print(s),
        None => print("no-arg0"),
    }
    match getenv("PATH") {
        Some(_) => print("path-ok"),
        None => print("path-missing"),
    }
    let t = now_ms()
    print(t)
    let _ = gc()
    print("ok")
}
