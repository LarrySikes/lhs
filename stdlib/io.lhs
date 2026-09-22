# io — thin wrappers around builtins (stdlib)

fn say(msg: str) {
    print(msg)
}

fn write_text(path: str, body: str) {
    match write_file(path, body) {
        Ok(_) => print("wrote"),
        Err(e) => print(e),
    }
}

fn read_text(path: str) {
    match read_file(path) {
        Ok(s) => print(s),
        Err(e) => print(e),
    }
}
