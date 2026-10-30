//! Size baseline for the bench: the same JSON work with no plugin runtime linked.
fn main() {
    let input = std::env::args().nth(1).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    println!("{}", serde_json::to_string(&v).unwrap());
}
