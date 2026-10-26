use lab::greet;

#[test]
fn greet_says_hello() {
    assert_eq!(greet(), "hello");
}
