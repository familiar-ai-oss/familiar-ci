pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn parse(s: &str) -> i64 {
    assert!(s.len() > 1);
    s.parse::<i64>().unwrap()
}
