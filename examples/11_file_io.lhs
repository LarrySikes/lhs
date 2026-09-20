# 11 — file I/O

fn main() {
    let path = "/tmp/np_io_demo.txt"
    match write_file(path, "hello from LHS") {
        Ok(_) => print("wrote"),
        Err(e) => print(e),
    }
    match read_file(path) {
        Ok(s) => print(s),
        Err(e) => print(e),
    }
}
