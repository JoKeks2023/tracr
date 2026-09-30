//! Prints one device sample: `cargo run -p tracr-platform --example sample`.
fn main() {
    let now = tracr_core::time::UtcMillis(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    );
    let s = tracr_platform::device_sensor().sample(now);
    println!("{s:?}");
    if let Some((sleep, wake)) = s.last_sleep_wake {
        println!("last sleep {sleep}  last wake {wake}");
    }
}
