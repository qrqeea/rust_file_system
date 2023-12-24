mod definitions;

use definitions::FileNode;

fn main() {

    let a = FileNode::Directory {
        name: String::from("/"), 
        files: vec![
            FileNode::File { name: String::from("target") },
            FileNode::Directory { 
                name: String::from("src"), 
                files: vec![FileNode::File { name: String::from("main.rs") }]}
        ]
    };

    a.list_all_files(String::from(""));

}