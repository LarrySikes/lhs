# Author: Larry Sikes
# Last modified: 2026-11-01
#
# http — helpers around http_get / http_post / http_request (needs curl)

fn fetch(url: str) {
    match http_get(url) {
        Ok(body) => print(body),
        Err(e) => print(e),
    }
}

fn post_json(url: str, body: str) {
    match http_post(url, body) {
        Ok(body) => print(body),
        Err(e) => print(e),
    }
}
