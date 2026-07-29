fn main() {
    println!("step 1: before Bonjour::new");
    let b = airplay_server::bonjour::Bonjour::new("test".to_string(), "AA:BB:CC:DD:EE:FF".to_string());
    println!("step 2: bonjour created");
    drop(b);
    println!("step 3: bonjour dropped");
}
