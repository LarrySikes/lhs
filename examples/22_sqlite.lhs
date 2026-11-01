# Author: Larry Sikes
# Last modified: 2026-11-01
#
# 22 — SQLite via sqlite3 CLI (db_exec / db_query)

fn main() {
    let path = "/tmp/lhs_demo.db"
    match db_exec(path, "CREATE TABLE IF NOT EXISTS note(id INTEGER PRIMARY KEY, msg TEXT);") {
        Ok(_) => print("table ok"),
        Err(e) => print(e),
    }
    match db_exec(path, "DELETE FROM note;") {
        Ok(_) => {},
        Err(e) => print(e),
    }
    match db_exec(path, "INSERT INTO note(msg) VALUES('hello from LHS');") {
        Ok(_) => print("insert ok"),
        Err(e) => print(e),
    }
    match db_query(path, "SELECT msg FROM note;") {
        Ok(json) => {
            match json_get(json, "0.msg") {
                Some(m) => print("row=" + m),
                None => print(json),
            }
        },
        Err(e) => print(e),
    }
}
