# 21 — regex + offline JSON (HTTP shown in apps/weather.lhs)

fn main() {
    let text = "order AB-42 shipped"
    match re_find(text, "[A-Z]+-[0-9]+") {
        Some(m) => print("found=" + m),
        None => print("not found"),
    }
    let cleaned = re_replace(text, "[0-9]+", "#")
    print(cleaned)

    let doc = "{\"id\":7,\"tag\":\"ok\"}"
    match json_get(doc, "tag") {
        Some(t) => print("tag=" + t),
        None => print("missing"),
    }
}
