# Author: Larry Sikes
# Last modified: 2026-11-01
#
# 20 — json_get on a fixed document (no network)

fn main() {
    let doc = "{\"main\":{\"temp\":300.15,\"humidity\":40},\"weather\":[{\"description\":\"clear sky\"}]}"
    match json_get(doc, "main.temp") {
        Some(t) => print("temp=" + t),
        None => print("missing temp"),
    }
    match json_get(doc, "weather.0.description") {
        Some(d) => print("desc=" + d),
        None => print("missing desc"),
    }
    match json_get(doc, "nope") {
        Some(_) => print("unexpected"),
        None => print("missing ok"),
    }
}
