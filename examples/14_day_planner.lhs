# 14 — day planner (functional demo, no stdin)
# Same model as apps/day_planner.lhs: todos by day, list/add/complete, save/load.

type Todo {
    Item { id: i32, done: i32, day: str, title: str },
}

type TodoList {
    Nil,
    Cons { head: Todo, tail: TodoList },
}

fn empty() -> TodoList {
    Nil
}

fn prepend(t: Todo, xs: TodoList) -> TodoList {
    Cons { head: t, tail: xs }
}

fn todo_id(t: Todo) -> i32 {
    match t {
        Item { id, done, day, title } => id,
    }
}

fn next_id(xs: TodoList) -> i32 {
    match xs {
        Nil => 1,
        Cons { head, tail } => {
            let a = todo_id(head)
            let b = next_id(tail)
            if a > b {
                a + 1
            } else {
                b + 1
            }
        },
    }
}

fn add_todo(xs: TodoList, day: str, title: str) -> TodoList {
    let id = next_id(xs)
    prepend(Item { id: id, done: 0, day: day, title: title }, xs)
}

fn mark_done(xs: TodoList, want: i32) -> TodoList {
    match xs {
        Nil => Nil,
        Cons { head, tail } => {
            match head {
                Item { id, done, day, title } => {
                    let h = if id == want {
                        Item { id: id, done: 1, day: day, title: title }
                    } else {
                        head
                    }
                    Cons { head: h, tail: mark_done(tail, want) }
                },
            }
        },
    }
}

fn count_open(xs: TodoList) -> i32 {
    match xs {
        Nil => 0,
        Cons { head, tail } => {
            match head {
                Item { id, done, day, title } => {
                    let n = if done == 0 {
                        1
                    } else {
                        0
                    }
                    n + count_open(tail)
                },
            }
        },
    }
}

fn show_one(t: Todo) {
    match t {
        Item { id, done, day, title } => {
            let mark = if done == 0 {
                "[ ]"
            } else {
                "[x]"
            }
            print(mark + " #" + id + "  " + day + "  " + title)
        },
    }
}

fn show_rest(xs: TodoList) {
    match xs {
        Nil => {},
        Cons { head, tail } => {
            show_one(head)
            show_rest(tail)
        },
    }
}

fn show_all(xs: TodoList) {
    match xs {
        Nil => print("(no todos)"),
        Cons { head, tail } => {
            show_one(head)
            show_rest(tail)
        },
    }
}

fn encode_one(t: Todo) -> str {
    match t {
        Item { id, done, day, title } => id + "|" + done + "|" + day + "|" + title,
    }
}

fn encode(xs: TodoList) -> str {
    match xs {
        Nil => "",
        Cons { head, tail } => encode_one(head) + "\n" + encode(tail),
    }
}

fn parse_int(s: str, i: i32, acc: i32) -> i32 {
    if i >= s.len() {
        return acc
    }
    let c = s[i]
    if c >= '0' && c <= '9' {
        parse_int(s, i + 1, acc * 10 + (c as i32 - '0' as i32))
    } else {
        acc
    }
}

fn field_at(s: str, start: i32, nth: i32) -> str {
    if nth == 0 {
        let bar = find_char(s, '|', start)
        if bar < 0 {
            str_slice(s, start, s.len())
        } else {
            str_slice(s, start, bar)
        }
    } else {
        let bar = find_char(s, '|', start)
        if bar < 0 {
            ""
        } else {
            field_at(s, bar + 1, nth - 1)
        }
    }
}

fn parse_line(s: str) -> Option<Todo> {
    if s.len() == 0 {
        return None
    }
    let id_s = field_at(s, 0, 0)
    let done_s = field_at(s, 0, 1)
    let day = field_at(s, 0, 2)
    let title = field_at(s, 0, 3)
    if id_s.len() == 0 {
        None
    } else {
        Some(Item {
            id: parse_int(id_s, 0, 0),
            done: parse_int(done_s, 0, 0),
            day: day,
            title: title,
        })
    }
}

fn parse_lines(s: str, i: i32, acc: TodoList) -> TodoList {
    if i >= s.len() {
        return acc
    }
    let nl = find_char(s, '\n', i)
    let end = if nl < 0 {
        s.len()
    } else {
        nl
    }
    let line = str_slice(s, i, end)
    let next = if nl < 0 {
        s.len()
    } else {
        nl + 1
    }
    match parse_line(line) {
        Some(t) => parse_lines(s, next, prepend(t, acc)),
        None => parse_lines(s, next, acc),
    }
}

fn main() {
    let path = "/tmp/lhs_day_planner_demo.txt"
    let a = add_todo(empty(), "2026-09-21", "Write overview")
    let b = add_todo(a, "2026-09-21", "Port day planner")
    let c = add_todo(b, "2026-09-22", "Cranelift methods")
    let d = mark_done(c, 2)
    print("open count: " + count_open(d))
    print("--- all ---")
    show_all(d)
    match write_file(path, encode(d)) {
        Ok(_) => print("wrote " + path),
        Err(e) => print(e),
    }
    match read_file(path) {
        Ok(s) => {
            let loaded = parse_lines(s, 0, empty())
            print("--- reloaded ---")
            show_all(loaded)
            print("open count: " + count_open(loaded))
        },
        Err(e) => print(e),
    }
}
