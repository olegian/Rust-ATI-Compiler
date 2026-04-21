fn main() {
    let a = 1;
    let b = &2;
    let c = &&3;
    same_value(a, b, c);

    // we invoke the function again, this time passing
    // a single value and references to the same value,
    // this should result in all parameters being observed
    // as being in the same AT.
    let d = 4;
    same_value(d, &d, &&d);

    let e = 5;
    let f = 6;
    returns_nested_ref(&&&&e, &&&f);

    let g = 7;
    let h = 8;
    returns_ref(&g, &h);

    let i = 9;
    let j = 10;
    compares_ref_to_value(i, &j);

    let k = 11;
    let l = 12;
    compares_ref_to_ref(&k, &l);
    
    let m = 13;
    let mut n = 14;
    compares_ref_to_ref_mut(&m, &mut n);

    let mut o = 15;
    let mut p = 16;
    compares_ref_mut_to_ref_mut(&mut o, &mut p);
}

fn same_value(a: u32, b: &u32, c: &&u32) -> u32 {
    a + *c
}

fn returns_ref<'a, 'b>(a: &'a u32, b: &'b u32) -> &'a u32 {
    a
}

fn returns_nested_ref<'a, 'b>(a: &'a &&&u32, b: &&&u32) -> &'a &'a &'a &'a u32 {
    a
}

fn compares_ref_to_value(a: u32, b: &u32) -> u32 {
    let tmp1 = if a == *b {
        1
    } else {
        2
    };

    let tmp2 = if *b < a {
        3
    } else {
        4
    };

    return tmp1 + tmp2;
}

fn compares_ref_to_ref(a: &u32, b: &u32) -> u32 {
    let tmp1 = if a < b {
        1
    } else {
        2
    };

    let tmp2 = if b < a {
        3
    } else {
        4
    };

    tmp1 + tmp2
}

fn compares_ref_to_ref_mut(a: &u32, b: &mut u32) -> u32 {
    let tmp1 = if a < b {
        1
    } else {
        2
    };

    // interestingly, b < a fails to compile (with no ParitalOrd for &mut)
    // which means the dereferences are necessary...
    let tmp2 = if *b < *a {
        3
    } else {
        4
    };

    tmp1 + tmp2
}

fn compares_ref_mut_to_ref_mut(a: &mut u32, b: &mut u32) -> u32 {
    let tmp1 = if a == b {
        1
    } else {
        2
    };

    let tmp2 = if b < a {
        3
    } else {
        4
    };

    tmp1 + tmp2
}
