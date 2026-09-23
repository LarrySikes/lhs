# mandelbrot — ASCII fractal (two-language demo)
#
# Numeric code people often prototype in Python then rewrite in C for speed.
# This stays in LHS and ships as a normal binary:
#   lhsc build apps/mandelbrot.lhs -o mandelbrot
#
# Run:
#   cargo run -p npc --bin lhsc -- run apps/mandelbrot.lhs
#   cargo run -p npc --bin lhsc -- run --jit apps/mandelbrot.lhs

fn escape(zr: f64, zi: f64, cr: f64, ci: f64, n: i32, maxn: i32) -> i32 {
    if n >= maxn {
        return maxn
    }
    if zr * zr + zi * zi > 4.0 {
        return n
    }
    let nr = zr * zr - zi * zi + cr
    let ni = 2.0 * zr * zi + ci
    escape(nr, ni, cr, ci, n + 1, maxn)
}

fn shade(n: i32, maxn: i32) -> str {
    if n >= maxn {
        return " "
    }
    if n > 20 {
        return "#"
    }
    if n > 12 {
        return "*"
    }
    if n > 6 {
        return "o"
    }
    if n > 2 {
        return "."
    }
    "+"
}

fn map_x(x: i32, w: i32) -> f64 {
    # real axis about -2.0 .. 1.0
    0.0 - 2.0 + (x as f64) * 3.0 / (w as f64)
}

fn map_y(y: i32, h: i32) -> f64 {
    # imag axis about -1.2 .. 1.2
    0.0 - 1.2 + (y as f64) * 2.4 / (h as f64)
}

fn cell(x: i32, y: i32, w: i32, h: i32, maxn: i32) -> str {
    let cr = map_x(x, w)
    let ci = map_y(y, h)
    shade(escape(0.0, 0.0, cr, ci, 0, maxn), maxn)
}

fn row(x: i32, y: i32, w: i32, h: i32, maxn: i32, acc: str) -> str {
    if x >= w {
        return acc
    }
    row(x + 1, y, w, h, maxn, acc + cell(x, y, w, h, maxn))
}

fn rows(y: i32, h: i32, w: i32, maxn: i32) {
    if y >= h {
        return
    }
    print(row(0, y, w, h, maxn, ""))
    rows(y + 1, h, w, maxn)
}

fn main() {
    let w = 72
    let h = 24
    let maxn = 40

    print("LHS mandelbrot " + w + "x" + h + " max=" + maxn)
    let t0 = now_ms()
    rows(0, h, w, maxn)
    let t1 = now_ms()
    print("ms=" + (t1 - t0))
    print("ok - build standalone with:")
    print("  lhsc build apps/mandelbrot.lhs -o mandelbrot")
}
