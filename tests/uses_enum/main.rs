#![allow(unused)]

enum Color {
    Red,
    Blue(u32),
}

enum Point {
    D1 { x: u32, y: f64 },
    D2 { a: f64, y: f64 },
}

#[ignore]
fn main() {
    use_color(Color::Blue(5), 10);
    use_color(Color::Red, 100);
    let a = 3.0;
    use_point(Point::D1 { x: 1, y: 2.0 }, a);
    use_point(Point::D2 { a: 10.0, y: 20.0 }, a);
}

fn use_color(c: Color, scale: u32) -> u32 {
    match c {
        Color::Red => scale,
        Color::Blue(i) => i + scale,
    }
}

fn use_point(p: Point, z: f64) -> f64 {
    match p {
        Point::D1 { x, y } => y + z,
        Point::D2 { a, y } => y + z,
    }
}
