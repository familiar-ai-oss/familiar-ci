use calc::calc::parse;

#[test]
fn parses() {
    assert_eq!(parse("x"), 1);
}
