#[path = "../../rust/config_store.rs"]
mod config_store;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root = std::path::Path::new(&args[2]);
    let mut store = config_store::Store::open(root, "iec104.master").unwrap();
    match args[1].as_str() {
        "read" => println!("{}", store.load("workspace").unwrap().unwrap()),
        "write" => {
            store.load("workspace").unwrap();
            let v = serde_json::from_str(&std::fs::read_to_string(&args[3]).unwrap()).unwrap();
            store.save("workspace", &v).unwrap();
        }
        _ => panic!(),
    }
}
