# json — thin helpers around json_get

fn show_field(doc: str, path: str) {
    match json_get(doc, path) {
        Some(v) => print(v),
        None => print("(missing)"),
    }
}
