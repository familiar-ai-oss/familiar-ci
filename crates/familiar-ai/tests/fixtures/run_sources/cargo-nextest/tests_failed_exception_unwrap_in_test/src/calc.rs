pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn parse(s: &str) -> i64 {
    s.parse::<i64>().unwrap()
}

pub fn parse_checked(_s: &str) -> Result<i64, String> {
    Err("todo".to_string())
}
