use calc::calc::parse_checked;

#[test]
fn parses() {
    let n = parse_checked("12").unwrap();
    assert_eq!(n, 12);
}
