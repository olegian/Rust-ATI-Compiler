mod rbtree;

fn main() {
    let mut tree: rbtree::RBTree<&'static str, u32> = rbtree::RBTree::new();

    tree.insert("AAA", 10);
    tree.insert("BBB", 20);
    tree.insert("CCC", 30);


    let a = tree.get(&"AAA").unwrap();
    let b = tree.get(&"BBB").unwrap();
    let c = tree.get(&"CCC").unwrap();

    let ret = foo(*a, *b, *c);
}

fn foo(a: u32, b: u32, unused: u32) -> u32 {
    a + b
}
