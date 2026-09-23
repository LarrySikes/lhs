# Perpetual Day Planner (LHS CLI)
# Mirrors the IWBasic day_planner feature set (no GUI):
#   - Day / Week / Month schedule views + prev/next/today/goto
#   - Appointments (time, title, notes) - add / update / delete
#   - Undated to-do list - add / done / remove
#   - Address book - name / phone / email
#   - save / load / print
#
#   cargo run -p npc --bin lhsc -- run apps/day_planner.lhs
#
# Data: /tmp/lhs_planner.dat  /tmp/lhs_contacts.dat

type Appt {
    Ev { id: i32, ymd: i32, hmm: i32, title: str, notes: str },
}

type ApptList {
    ANil,
    ACons { head: Appt, tail: ApptList },
}

type Todo {
    Item { id: i32, done: i32, title: str },
}

type TodoList {
    Nil,
    Cons { head: Todo, tail: TodoList },
}

type Contact {
    Person { id: i32, name: str, phone: str, email: str },
}

type ContactList {
    CNil,
    CCons { head: Contact, tail: ContactList },
}

# ---- tiny helpers ----

fn empty_a() -> ApptList { ANil }
fn empty_t() -> TodoList { Nil }
fn empty_c() -> ContactList { CNil }

fn prompt(msg: str) -> str {
    print(msg)
    match read_line() {
        Some(s) => s,
        None => "",
    }
}

# Lowercase A-Z; leave everything else alone.
fn lower_char(c: char) -> char {
    if c >= 'A' && c <= 'Z' {
        (c as i32 + 32) as char
    } else {
        c
    }
}

fn lower_str(s: str, i: i32, acc: str) -> str {
    if i >= s.len() {
        return acc
    }
    lower_str(s, i + 1, acc + lower_char(s[i]))
}

# Drop leading/trailing spaces.
fn trim_left(s: str, i: i32) -> str {
    if i >= s.len() {
        return ""
    }
    if s[i] == ' ' || s[i] == '\t' {
        trim_left(s, i + 1)
    } else {
        str_slice(s, i, s.len())
    }
}

fn trim_right(s: str) -> str {
    if s.len() == 0 {
        return ""
    }
    let last = s.len() - 1
    if s[last] == ' ' || s[last] == '\t' {
        trim_right(str_slice(s, 0, last))
    } else {
        s
    }
}

fn trim(s: str) -> str {
    trim_right(trim_left(s, 0))
}

# Accept "month", "view month", "VIEW Month", etc.
fn starts_with(s: str, prefix: str) -> i32 {
    if s.len() < prefix.len() {
        return 0
    }
    if str_slice(s, 0, prefix.len()) == prefix {
        1
    } else {
        0
    }
}

fn normalize_cmd(raw: str) -> str {
    let s = trim(lower_str(raw, 0, ""))
    if starts_with(s, "view ") == 1 {
        trim(str_slice(s, 5, s.len()))
    } else {
        s
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

fn pad2(n: i32) -> str {
    if n < 10 {
        "0" + n
    } else {
        "" + n
    }
}

fn ymd_of(y: i32, m: i32, d: i32) -> i32 {
    y * 10000 + m * 100 + d
}

fn rem(a: i32, b: i32) -> i32 {
    a - (a / b) * b
}

fn y_of(ymd: i32) -> i32 { ymd / 10000 }
fn m_of(ymd: i32) -> i32 { rem(ymd / 100, 100) }
fn d_of(ymd: i32) -> i32 { rem(ymd, 100) }

fn is_leap(y: i32) -> i32 {
    if rem(y, 400) == 0 {
        1
    } else {
        if rem(y, 100) == 0 {
            0
        } else {
            if rem(y, 4) == 0 {
                1
            } else {
                0
            }
        }
    }
}

fn dim(y: i32, m: i32) -> i32 {
    if m == 1 || m == 3 || m == 5 || m == 7 || m == 8 || m == 10 || m == 12 {
        31
    } else {
        if m == 2 {
            if is_leap(y) == 1 {
                29
            } else {
                28
            }
        } else {
            30
        }
    }
}

# Sakamoto dow: 0=Sunday
fn dow(y: i32, m: i32, d: i32) -> i32 {
    let t0 = 0
    let t1 = 3
    let t2 = if is_leap(y) == 1 { 2 } else { 3 }
    let t3 = 6
    let t4 = 1
    let t5 = 4
    let t6 = 6
    let t7 = 2
    let t8 = 5
    let t9 = 0
    let t10 = 3
    let t11 = 5
    let yy = if m < 3 { y - 1 } else { y }
    let t = if m == 1 { t0 } else { if m == 2 { t1 } else { if m == 3 { t2 } else { if m == 4 { t3 } else { if m == 5 { t4 } else { if m == 6 { t5 } else { if m == 7 { t6 } else { if m == 8 { t7 } else { if m == 9 { t8 } else { if m == 10 { t9 } else { if m == 11 { t10 } else { t11 } } } } } } } } } } }
    (rem(yy + yy / 4 - yy / 100 + yy / 400 + t + d, 7))
}

fn day_name(w: i32) -> str {
    if w == 0 { "Sun" } else { if w == 1 { "Mon" } else { if w == 2 { "Tue" } else { if w == 3 { "Wed" } else { if w == 4 { "Thu" } else { if w == 5 { "Fri" } else { "Sat" } } } } } }
}

# 0=Sun .. 6=Sat, or -1 if not a weekday name
fn weekday_num(s: str) -> i32 {
    if s == "sunday" || s == "sun" { 0 } else {
    if s == "monday" || s == "mon" { 1 } else {
    if s == "tuesday" || s == "tue" || s == "tues" { 2 } else {
    if s == "wednesday" || s == "wed" { 3 } else {
    if s == "thursday" || s == "thu" || s == "thur" || s == "thurs" { 4 } else {
    if s == "friday" || s == "fri" { 5 } else {
    if s == "saturday" || s == "sat" { 6 } else { -1 }
    } } } } } }
}

# 1..12, or -1 if not a month name
fn month_num(s: str) -> i32 {
    if s == "january" || s == "jan" { 1 } else {
    if s == "february" || s == "feb" { 2 } else {
    if s == "march" || s == "mar" { 3 } else {
    if s == "april" || s == "apr" { 4 } else {
    if s == "may" { 5 } else {
    if s == "june" || s == "jun" { 6 } else {
    if s == "july" || s == "jul" { 7 } else {
    if s == "august" || s == "aug" { 8 } else {
    if s == "september" || s == "sep" || s == "sept" { 9 } else {
    if s == "october" || s == "oct" { 10 } else {
    if s == "november" || s == "nov" { 11 } else {
    if s == "december" || s == "dec" { 12 } else { -1 }
    } } } } } } } } } } }
}

# Jump to that weekday in the same week as cur (Sun..Sat).
fn goto_weekday(cur: i32, want: i32) -> i32 {
    let w = dow(y_of(cur), m_of(cur), d_of(cur))
    add_days(cur, want - w)
}

# Jump to the 1st of month m in cur's year (clamp day if needed).
fn goto_month(cur: i32, m: i32) -> i32 {
    let y = y_of(cur)
    let d = d_of(cur)
    let md = dim(y, m)
    let day = if d > md { md } else { d }
    ymd_of(y, m, day)
}

fn fmt_ymd(ymd: i32) -> str {
    y_of(ymd) + "-" + pad2(m_of(ymd)) + "-" + pad2(d_of(ymd))
}

fn fmt_hmm(hmm: i32) -> str {
    pad2(hmm / 100) + ":" + pad2(rem(hmm, 100))
}

fn add_days(ymd: i32, n: i32) -> i32 {
    let y = y_of(ymd)
    let m = m_of(ymd)
    let d = d_of(ymd) + n
    norm_date(y, m, d)
}

fn norm_date(y: i32, m: i32, d: i32) -> i32 {
    if d < 1 {
        if m == 1 {
            norm_date(y - 1, 12, d + dim(y - 1, 12))
        } else {
            norm_date(y, m - 1, d + dim(y, m - 1))
        }
    } else {
        let md = dim(y, m)
        if d > md {
            if m == 12 {
                norm_date(y + 1, 1, d - md)
            } else {
                norm_date(y, m + 1, d - md)
            }
        } else {
            ymd_of(y, m, d)
        }
    }
}

fn parse_ymd(s: str) -> i32 {
    # accepts YYYY-MM-DD or YYYYMMDD
    if s.len() >= 10 {
        if s[4] == '-' {
            let y = parse_int(str_slice(s, 0, 4), 0, 0)
            let m = parse_int(str_slice(s, 5, 7), 0, 0)
            let d = parse_int(str_slice(s, 8, 10), 0, 0)
            return ymd_of(y, m, d)
        }
    }
    parse_int(s, 0, 0)
}

# M/D or M/D/YYYY (also MM/DD). Returns 0 if not that shape.
fn parse_slash_date(s: str, year: i32) -> i32 {
    let slash = find_char(s, '/', 0)
    if slash < 0 {
        return 0
    }
    let m = parse_int(str_slice(s, 0, slash), 0, 0)
    let rest = str_slice(s, slash + 1, s.len())
    let slash2 = find_char(rest, '/', 0)
    if slash2 < 0 {
        let d = parse_int(rest, 0, 0)
        if m < 1 || m > 12 || d < 1 || d > dim(year, m) {
            0
        } else {
            ymd_of(year, m, d)
        }
    } else {
        let d = parse_int(str_slice(rest, 0, slash2), 0, 0)
        let y = parse_int(str_slice(rest, slash2 + 1, rest.len()), 0, 0)
        if y < 1 || m < 1 || m > 12 || d < 1 || d > dim(y, m) {
            0
        } else {
            ymd_of(y, m, d)
        }
    }
}

# "september 21" / "sep 21" — day of that month in year. Returns 0 if not.
fn parse_month_day_words(s: str, year: i32) -> i32 {
    let sp = find_char(s, ' ', 0)
    if sp < 0 {
        return 0
    }
    let name = str_slice(s, 0, sp)
    let daypart = trim(str_slice(s, sp + 1, s.len()))
    # no extra words after the day number
    let sp2 = find_char(daypart, ' ', 0)
    if sp2 >= 0 {
        return 0
    }
    let m = month_num(name)
    let d = parse_int(daypart, 0, 0)
    if m < 1 || d < 1 || d > dim(year, m) {
        0
    } else {
        ymd_of(year, m, d)
    }
}

# Any supported date phrase. Uses cur's year when year omitted. 0 = fail.
fn parse_any_date(s: str, cur: i32) -> i32 {
    let t = trim(s)
    if t.len() == 0 {
        return 0
    }
    if t.len() >= 10 {
        if t[4] == '-' {
            return parse_ymd(t)
        }
    }
    let y = y_of(cur)
    let a = parse_slash_date(t, y)
    if a != 0 {
        return a
    }
    let b = parse_month_day_words(t, y)
    if b != 0 {
        return b
    }
    0
}

fn parse_hmm(s: str) -> i32 {
    if s.len() >= 5 {
        if s[2] == ':' {
            return parse_int(str_slice(s, 0, 2), 0, 0) * 100 + parse_int(str_slice(s, 3, 5), 0, 0)
        }
    }
    parse_int(s, 0, 0)
}

# ---- appointments ----

fn a_id(a: Appt) -> i32 {
    match a { Ev { id, ymd, hmm, title, notes } => id, }
}

fn next_aid(xs: ApptList) -> i32 {
    match xs {
        ANil => 1,
        ACons { head, tail } => {
            let a = a_id(head)
            let b = next_aid(tail)
            if a > b { a + 1 } else { b + 1 }
        },
    }
}

fn a_add(xs: ApptList, ymd: i32, hmm: i32, title: str, notes: str) -> ApptList {
    ACons {
        head: Ev { id: next_aid(xs), ymd: ymd, hmm: hmm, title: title, notes: notes },
        tail: xs,
    }
}

fn a_upd(xs: ApptList, want: i32, ymd: i32, hmm: i32, title: str, notes: str) -> ApptList {
    match xs {
        ANil => ANil,
        ACons { head, tail } => {
            match head {
                Ev { id, ymd: oy, hmm: oh, title: ot, notes: on } => {
                    let h = if id == want {
                        Ev { id: id, ymd: ymd, hmm: hmm, title: title, notes: notes }
                    } else {
                        head
                    }
                    ACons { head: h, tail: a_upd(tail, want, ymd, hmm, title, notes) }
                },
            }
        },
    }
}

fn a_del(xs: ApptList, want: i32) -> ApptList {
    match xs {
        ANil => ANil,
        ACons { head, tail } => {
            if a_id(head) == want {
                a_del(tail, want)
            } else {
                ACons { head: head, tail: a_del(tail, want) }
            }
        },
    }
}

fn a_show_one(a: Appt) {
    match a {
        Ev { id, ymd, hmm, title, notes } => {
            print("  #" + id + "  " + fmt_hmm(hmm) + "  " + title + "  [" + notes + "]")
        },
    }
}

fn a_show_day(xs: ApptList, ymd: i32, found: i32) -> i32 {
    match xs {
        ANil => found,
        ACons { head, tail } => {
            match head {
                Ev { id, ymd: yd, hmm, title, notes } => {
                    if yd == ymd {
                        a_show_one(head)
                        a_show_day(tail, ymd, found + 1)
                    } else {
                        a_show_day(tail, ymd, found)
                    }
                },
            }
        },
    }
}

fn show_day_view(xs: ApptList, ymd: i32) {
    let y = y_of(ymd)
    let m = m_of(ymd)
    let d = d_of(ymd)
    print("=== DAY " + fmt_ymd(ymd) + " (" + day_name(dow(y, m, d)) + ") ===")
    let n = a_show_day(xs, ymd, 0)
    if n == 0 {
        print("  (no appointments - use aadd)")
    }
}

fn show_week_view(xs: ApptList, ymd: i32, i: i32) {
    if i == 0 {
        print("=== WEEK starting " + fmt_ymd(ymd) + " ===")
    }
    if i < 7 {
        let day = add_days(ymd, i)
        print("-- " + fmt_ymd(day) + " " + day_name(dow(y_of(day), m_of(day), d_of(day))))
        let n = a_show_day(xs, day, 0)
        if n == 0 {
            print("  (free)")
        }
        show_week_view(xs, ymd, i + 1)
    }
}

fn show_month_cells(xs: ApptList, y: i32, m: i32, d: i32, last: i32) {
    if d <= last {
        let ymd = ymd_of(y, m, d)
        let n = count_day(xs, ymd, 0)
        let mark = if n > 0 { "*" } else { " " }
        print("  " + pad2(d) + mark + " " + day_name(dow(y, m, d)) + "  (" + n + " appt)")
        show_month_cells(xs, y, m, d + 1, last)
    }
}

fn count_day(xs: ApptList, ymd: i32, acc: i32) -> i32 {
    match xs {
        ANil => acc,
        ACons { head, tail } => {
            match head {
                Ev { id, ymd: yd, hmm, title, notes } => {
                    let a = if yd == ymd { 1 } else { 0 }
                    count_day(tail, ymd, acc + a)
                },
            }
        },
    }
}

fn show_month_view(xs: ApptList, ymd: i32) {
    let y = y_of(ymd)
    let m = m_of(ymd)
    print("=== MONTH " + y + "-" + pad2(m) + " ===")
    show_month_cells(xs, y, m, 1, dim(y, m))
}

fn show_schedule(view: i32, cur: i32, xs: ApptList) {
    if view == 0 {
        show_day_view(xs, cur)
    } else {
        if view == 1 {
            # week starts on Sunday of current week
            let w = dow(y_of(cur), m_of(cur), d_of(cur))
            show_week_view(xs, add_days(cur, 0 - w), 0)
        } else {
            show_month_view(xs, cur)
        }
    }
}

fn encode_a(xs: ApptList) -> str {
    match xs {
        ANil => "",
        ACons { head, tail } => {
            match head {
                Ev { id, ymd, hmm, title, notes } => {
                    "A|" + id + "|" + ymd + "|" + hmm + "|" + title + "|" + notes + "\n" + encode_a(tail)
                },
            }
        },
    }
}

fn parse_a_line(s: str) -> Option<Appt> {
    if s.len() == 0 {
        return None
    }
    if s[0] != 'A' {
        return None
    }
    Some(Ev {
        id: parse_int(field_at(s, 0, 1), 0, 0),
        ymd: parse_int(field_at(s, 0, 2), 0, 0),
        hmm: parse_int(field_at(s, 0, 3), 0, 0),
        title: field_at(s, 0, 4),
        notes: field_at(s, 0, 5),
    })
}

# ---- todos (undated, like original) ----

fn t_id(t: Todo) -> i32 {
    match t { Item { id, done, title } => id, }
}

fn next_tid(xs: TodoList) -> i32 {
    match xs {
        Nil => 1,
        Cons { head, tail } => {
            let a = t_id(head)
            let b = next_tid(tail)
            if a > b { a + 1 } else { b + 1 }
        },
    }
}

fn t_add(xs: TodoList, title: str) -> TodoList {
    Cons { head: Item { id: next_tid(xs), done: 0, title: title }, tail: xs }
}

fn t_done(xs: TodoList, want: i32) -> TodoList {
    match xs {
        Nil => Nil,
        Cons { head, tail } => {
            match head {
                Item { id, done, title } => {
                    let h = if id == want {
                        Item { id: id, done: 1, title: title }
                    } else {
                        head
                    }
                    Cons { head: h, tail: t_done(tail, want) }
                },
            }
        },
    }
}

fn t_del(xs: TodoList, want: i32) -> TodoList {
    match xs {
        Nil => Nil,
        Cons { head, tail } => {
            if t_id(head) == want {
                t_del(tail, want)
            } else {
                Cons { head: head, tail: t_del(tail, want) }
            }
        },
    }
}

fn t_show(xs: TodoList) {
    match xs {
        Nil => print("(no tasks)"),
        Cons { head, tail } => {
            match head {
                Item { id, done, title } => {
                    let mark = if done == 0 { "[ ]" } else { "[x]" }
                    print(mark + " #" + id + "  " + title)
                    t_show_rest(tail)
                },
            }
        },
    }
}

fn t_show_rest(xs: TodoList) {
    match xs {
        Nil => {},
        Cons { head, tail } => {
            match head {
                Item { id, done, title } => {
                    let mark = if done == 0 { "[ ]" } else { "[x]" }
                    print(mark + " #" + id + "  " + title)
                    t_show_rest(tail)
                },
            }
        },
    }
}

fn encode_t(xs: TodoList) -> str {
    match xs {
        Nil => "",
        Cons { head, tail } => {
            match head {
                Item { id, done, title } => "T|" + id + "|" + done + "|" + title + "\n" + encode_t(tail),
            }
        },
    }
}

fn parse_t_line(s: str) -> Option<Todo> {
    if s.len() == 0 {
        return None
    }
    if s[0] != 'T' {
        return None
    }
    Some(Item {
        id: parse_int(field_at(s, 0, 1), 0, 0),
        done: parse_int(field_at(s, 0, 2), 0, 0),
        title: field_at(s, 0, 3),
    })
}

# ---- contacts ----

fn c_id(c: Contact) -> i32 {
    match c { Person { id, name, phone, email } => id, }
}

fn next_cid(xs: ContactList) -> i32 {
    match xs {
        CNil => 1,
        CCons { head, tail } => {
            let a = c_id(head)
            let b = next_cid(tail)
            if a > b { a + 1 } else { b + 1 }
        },
    }
}

fn c_add(xs: ContactList, name: str, phone: str, email: str) -> ContactList {
    CCons {
        head: Person { id: next_cid(xs), name: name, phone: phone, email: email },
        tail: xs,
    }
}

fn c_del(xs: ContactList, want: i32) -> ContactList {
    match xs {
        CNil => CNil,
        CCons { head, tail } => {
            if c_id(head) == want {
                c_del(tail, want)
            } else {
                CCons { head: head, tail: c_del(tail, want) }
            }
        },
    }
}

fn c_show(xs: ContactList) {
    match xs {
        CNil => print("(no contacts)"),
        CCons { head, tail } => {
            match head {
                Person { id, name, phone, email } => {
                    print("#" + id + "  " + name + "  " + phone + "  " + email)
                    c_show_rest(tail)
                },
            }
        },
    }
}

fn c_show_rest(xs: ContactList) {
    match xs {
        CNil => {},
        CCons { head, tail } => {
            match head {
                Person { id, name, phone, email } => {
                    print("#" + id + "  " + name + "  " + phone + "  " + email)
                    c_show_rest(tail)
                },
            }
        },
    }
}

fn encode_c(xs: ContactList) -> str {
    match xs {
        CNil => "",
        CCons { head, tail } => {
            match head {
                Person { id, name, phone, email } => {
                    "C|" + id + "|" + name + "|" + phone + "|" + email + "\n" + encode_c(tail)
                },
            }
        },
    }
}

fn parse_c_line(s: str) -> Option<Contact> {
    if s.len() == 0 {
        return None
    }
    if s[0] != 'C' {
        return None
    }
    Some(Person {
        id: parse_int(field_at(s, 0, 1), 0, 0),
        name: field_at(s, 0, 2),
        phone: field_at(s, 0, 3),
        email: field_at(s, 0, 4),
    })
}

# ---- file I/O ----

fn load_lines_a(s: str, i: i32, acc: ApptList) -> ApptList {
    if i >= s.len() {
        return acc
    }
    let nl = find_char(s, '\n', i)
    let end = if nl < 0 { s.len() } else { nl }
    let line = str_slice(s, i, end)
    let next = if nl < 0 { s.len() } else { nl + 1 }
    match parse_a_line(line) {
        Some(a) => load_lines_a(s, next, ACons { head: a, tail: acc }),
        None => load_lines_a(s, next, acc),
    }
}

fn load_lines_t(s: str, i: i32, acc: TodoList) -> TodoList {
    if i >= s.len() {
        return acc
    }
    let nl = find_char(s, '\n', i)
    let end = if nl < 0 { s.len() } else { nl }
    let line = str_slice(s, i, end)
    let next = if nl < 0 { s.len() } else { nl + 1 }
    match parse_t_line(line) {
        Some(t) => load_lines_t(s, next, Cons { head: t, tail: acc }),
        None => load_lines_t(s, next, acc),
    }
}

fn load_lines_c(s: str, i: i32, acc: ContactList) -> ContactList {
    if i >= s.len() {
        return acc
    }
    let nl = find_char(s, '\n', i)
    let end = if nl < 0 { s.len() } else { nl }
    let line = str_slice(s, i, end)
    let next = if nl < 0 { s.len() } else { nl + 1 }
    match parse_c_line(line) {
        Some(c) => load_lines_c(s, next, CCons { head: c, tail: acc }),
        None => load_lines_c(s, next, acc),
    }
}

fn save_all(ap: ApptList, td: TodoList, ct: ContactList) {
    let body = encode_a(ap) + encode_t(td)
    match write_file("/tmp/lhs_planner.dat", body) {
        Ok(_) => print("saved /tmp/lhs_planner.dat"),
        Err(e) => print("save planner failed: " + e),
    }
    match write_file("/tmp/lhs_contacts.dat", encode_c(ct)) {
        Ok(_) => print("saved /tmp/lhs_contacts.dat"),
        Err(e) => print("save contacts failed: " + e),
    }
}

fn print_dump(view: i32, cur: i32, ap: ApptList, td: TodoList) {
    print("======== PRINT ========")
    show_schedule(view, cur, ap)
    print("--- TASKS ---")
    t_show(td)
    let buf = encode_a(ap) + encode_t(td)
    match write_file("/tmp/lhs_planner_print.txt", buf) {
        Ok(_) => print("(also wrote /tmp/lhs_planner_print.txt)"),
        Err(e) => print(e),
    }
}

fn help() {
    print("Perpetual Day Planner - commands:")
    print("  (case does not matter; you may type 'view ' before a view command)")
    print("  VIEW:   day | week | month | prev | next | today | goto")
    print("         examples:  month   OR   view month")
    print("  JUMP:   monday .. sunday   (or mon, tue, ...)")
    print("          january .. december  (or jan, feb, sep, ...)")
    print("          september 21   |   9/21   |   2026-09-21")
    print("          goto 9/21      |   view september 21")
    print("  APPTS:  sched | aadd | aupd | adel")
    print("  TASKS:  tasks | tadd | tdone | tdel")
    print("  BOOK:   contacts | cadd | cdel")
    print("  FILE:   save | load | print | help | quit")
}

fn view_name(v: i32) -> str {
    if v == 0 { "day" } else { if v == 1 { "week" } else { "month" } }
}

fn repl(view: i32, cur: i32, ap: ApptList, td: TodoList, ct: ContactList) {
    print("--- " + view_name(view) + " | " + fmt_ymd(cur) + " ---")
    let line = normalize_cmd(prompt("> "))
    if line == "quit" || line == "q" {
        save_all(ap, td, ct)
        print("bye")
        return
    }
    if line == "help" || line == "h" {
        help()
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "day" {
        show_schedule(0, cur, ap)
        repl(0, cur, ap, td, ct)
        return
    }
    if line == "week" {
        show_schedule(1, cur, ap)
        repl(1, cur, ap, td, ct)
        return
    }
    if line == "month" {
        show_schedule(2, cur, ap)
        repl(2, cur, ap, td, ct)
        return
    }
    if line == "today" {
        let t = ymd_of(2026, 9, 21)
        show_schedule(view, t, ap)
        repl(view, t, ap, td, ct)
        return
    }
    # weekday name -> that day this week, day view
    let wd = weekday_num(line)
    if wd >= 0 {
        let n = goto_weekday(cur, wd)
        show_schedule(0, n, ap)
        repl(0, n, ap, td, ct)
        return
    }
    # month name -> that month (same year), month view
    let mn = month_num(line)
    if mn >= 1 {
        let n = goto_month(cur, mn)
        show_schedule(2, n, ap)
        repl(2, n, ap, td, ct)
        return
    }
    if line == "prev" {
        let step = if view == 2 { -30 } else { if view == 1 { -7 } else { -1 } }
        let n = add_days(cur, step)
        show_schedule(view, n, ap)
        repl(view, n, ap, td, ct)
        return
    }
    if line == "next" {
        let step = if view == 2 { 30 } else { if view == 1 { 7 } else { 1 } }
        let n = add_days(cur, step)
        show_schedule(view, n, ap)
        repl(view, n, ap, td, ct)
        return
    }
    # goto with date on same line: "goto 9/21"
    if starts_with(line, "goto ") == 1 {
        let arg = trim(str_slice(line, 5, line.len()))
        let n = parse_any_date(arg, cur)
        if n != 0 {
            show_schedule(0, n, ap)
            repl(0, n, ap, td, ct)
            return
        }
        print("bad date - try 9/21 or september 21 or 2026-09-21")
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "goto" {
        let s = prompt("date (9/21 or YYYY-MM-DD or september 21): ")
        let n = parse_any_date(normalize_cmd(s), cur)
        if n != 0 {
            show_schedule(0, n, ap)
            repl(0, n, ap, td, ct)
            return
        }
        print("bad date - try 9/21 or september 21 or 2026-09-21")
        repl(view, cur, ap, td, ct)
        return
    }
    # bare date: "september 21", "9/21", "2026-09-21"
    let jump = parse_any_date(line, cur)
    if jump != 0 {
        show_schedule(0, jump, ap)
        repl(0, jump, ap, td, ct)
        return
    }
    if line == "sched" {
        show_schedule(view, cur, ap)
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "aadd" {
        let tm = prompt("time HH:MM: ")
        let title = prompt("title: ")
        let notes = prompt("notes: ")
        let ys = a_add(ap, cur, parse_hmm(tm), title, notes)
        print("appointment added")
        show_day_view(ys, cur)
        repl(view, cur, ys, td, ct)
        return
    }
    if line == "aupd" {
        let id = parse_int(prompt("appt id: "), 0, 0)
        let tm = prompt("time HH:MM: ")
        let title = prompt("title: ")
        let notes = prompt("notes: ")
        let ys = a_upd(ap, id, cur, parse_hmm(tm), title, notes)
        print("appointment updated")
        repl(view, cur, ys, td, ct)
        return
    }
    if line == "adel" {
        let id = parse_int(prompt("appt id: "), 0, 0)
        let ys = a_del(ap, id)
        print("appointment deleted")
        repl(view, cur, ys, td, ct)
        return
    }
    if line == "tasks" {
        print("=== TASKS (undated) ===")
        t_show(td)
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "tadd" {
        let title = prompt("task: ")
        let ys = t_add(td, title)
        print("task added")
        repl(view, cur, ap, ys, ct)
        return
    }
    if line == "tdone" {
        let id = parse_int(prompt("task id: "), 0, 0)
        repl(view, cur, ap, t_done(td, id), ct)
        return
    }
    if line == "tdel" {
        let id = parse_int(prompt("task id: "), 0, 0)
        repl(view, cur, ap, t_del(td, id), ct)
        return
    }
    if line == "contacts" {
        print("=== ADDRESS BOOK ===")
        c_show(ct)
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "cadd" {
        let name = prompt("name: ")
        let phone = prompt("phone: ")
        let email = prompt("email: ")
        repl(view, cur, ap, td, c_add(ct, name, phone, email))
        return
    }
    if line == "cdel" {
        let id = parse_int(prompt("contact id: "), 0, 0)
        repl(view, cur, ap, td, c_del(ct, id))
        return
    }
    if line == "save" {
        save_all(ap, td, ct)
        repl(view, cur, ap, td, ct)
        return
    }
    if line == "load" {
        let ap2 = match read_file("/tmp/lhs_planner.dat") {
            Ok(s) => load_lines_a(s, 0, empty_a()),
            Err(_) => empty_a(),
        }
        let td2 = match read_file("/tmp/lhs_planner.dat") {
            Ok(s) => load_lines_t(s, 0, empty_t()),
            Err(_) => empty_t(),
        }
        let ct2 = match read_file("/tmp/lhs_contacts.dat") {
            Ok(s) => load_lines_c(s, 0, empty_c()),
            Err(_) => empty_c(),
        }
        print("reloaded")
        repl(view, cur, ap2, td2, ct2)
        return
    }
    if line == "print" {
        print_dump(view, cur, ap, td)
        repl(view, cur, ap, td, ct)
        return
    }
    print("unknown - try help")
    repl(view, cur, ap, td, ct)
}

fn seed_if_empty(ap: ApptList, td: TodoList, ct: ContactList) -> i32 {
    match ap {
        ANil => {
            match td {
                Nil => 1,
                Cons { head, tail } => 0,
            }
        },
        ACons { head, tail } => 0,
    }
}

fn main() {
    let today = ymd_of(2026, 9, 21)
    let ap0 = match read_file("/tmp/lhs_planner.dat") {
        Ok(s) => load_lines_a(s, 0, empty_a()),
        Err(_) => empty_a(),
    }
    let td0 = match read_file("/tmp/lhs_planner.dat") {
        Ok(s) => load_lines_t(s, 0, empty_t()),
        Err(_) => empty_t(),
    }
    let ct0 = match read_file("/tmp/lhs_contacts.dat") {
        Ok(s) => load_lines_c(s, 0, empty_c()),
        Err(_) => empty_c(),
    }
    let ap = if seed_if_empty(ap0, td0, ct0) == 1 {
        a_add(empty_a(), today, 900, "Standup", "LHS planner port")
    } else {
        ap0
    }
    let td = if seed_if_empty(ap0, td0, ct0) == 1 {
        t_add(t_add(empty_t(), "Review day/week/month views"), "Fill address book")
    } else {
        td0
    }
    let ct = if seed_if_empty(ap0, td0, ct0) == 1 {
        c_add(empty_c(), "Ada Lovelace", "555-0100", "ada.at.analytical.engine")
    } else {
        ct0
    }
    help()
    show_schedule(0, today, ap)
    print("--- TASKS ---")
    t_show(td)
    print("--- CONTACTS ---")
    c_show(ct)
    repl(0, today, ap, td, ct)
}
