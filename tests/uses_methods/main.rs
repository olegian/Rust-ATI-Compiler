#![allow(unused)]

struct Counter {
    val: u32,
    unused: u32, // should stay in it's own AT
}

impl Counter {
    fn new(initial: u32, unused_param: u32) -> Counter {
        Counter {
            val: initial,
            unused: 0,
        }
    }

    fn add_1(&self, amount: u32, unused_param: u32) -> u32 {
        self.val + amount
    }

    fn add_2(&mut self, amount: u32, unused_param: u32) {
        self.val = self.val + amount;
    }

    fn add_3(self, unused_param: u32) -> Self {
        self
    }
}

#[ignore]
fn main() {
    let mut c = Counter::new(5, 99);
    let r = c.add_1(3, 10);
    c.add_2(2, 99);
    c.add_3(99);
}
