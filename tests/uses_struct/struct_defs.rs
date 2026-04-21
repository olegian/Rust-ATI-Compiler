pub struct Struct {
    pub a: u64,
    pub b: bool,
    pub c: Inner,
}

pub struct TupleStruct(pub u32, pub bool, pub Inner);

// specifically a clashing name in a different file from main
pub struct Inner {
    x: u64,
    b: u8,
}

impl Inner {
    pub fn new(x: u64, b: u8) -> Self {
        Inner { x, b }
    }

    pub fn add_x(&mut self, x: u64) {
        self.x += x;
    }
}

pub struct ContainsRef<'a, 'b, 'c> {
    pub a: i64,
    pub b: &'a i32,
    pub c: &'b mut i64,
    pub d: &'c &'c &'c i64,
}
