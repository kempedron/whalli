use whalli::value::Value;

#[test]
fn test_value_size_compacted() {
    let size = std::mem::size_of::<Value>();
    println!("Value size: {} bytes", size);
    assert_eq!(size, 16, "Expected Value to be compacted to 16 bytes");
}
