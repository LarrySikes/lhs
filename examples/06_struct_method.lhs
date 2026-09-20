# 06 — structs and methods

type Point {
    x: f64,
    y: f64,
}

fn Point.dist(self, other: Point) -> f64 {
    let dx = self.x - other.x
    let dy = self.y - other.y
    (dx * dx + dy * dy).sqrt()
}

fn main() {
    let a = Point { x: 0.0, y: 0.0 }
    let b = Point { x: 3.0, y: 4.0 }
    print(a.dist(b))
}
