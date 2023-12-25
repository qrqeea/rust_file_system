mod definitions;

use definitions::*;

fn main() {

    let mut root = generate_diorectory("".to_string(), "".to_string());
    root.add_file(generate_file("Cargo.lock".to_string(), "111".to_string(), "".to_string()));
    root.add_file(generate_file("Cargo.toml".to_string(), "222".to_string(), "".to_string()));
    root.add_file(generate_diorectory("target".to_string(), "".to_string()));
    let src = root.add_file(generate_diorectory("src".to_string(), "".to_string())).unwrap();

    src.add_file(generate_file("main.rs".to_string(), "hello world".to_string(), "/src".to_string()));
    
    root.list_all_files("".to_string());
    // src.list_all_files("/".to_string());

}