# 05 — algebraic data types

type Shape {
    Circle { r: f64 },
    Rect { w: f64, h: f64 },
}

fn area(s: Shape) -> f64 {
    match s {
        Circle { r } => 3.141592653589793 * r * r,
        Rect { w, h } => w * h,
    }
}

fn main() {
    print(area(Circle { r: 2.0 }))
    print(area(Rect { w: 3.0, h: 4.0 }))
}
