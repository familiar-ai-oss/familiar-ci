use calc::calc::add;

extern "C" {
    fn raise(sig: i32) -> i32;
}

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}

#[test]
fn crashes() {
    unsafe { raise(9); }
}
