
pub enum FileNode {
    // 普通文件
    File {
        name: String,
    },
    // 目录文件
    Directory {
        name: String,
        files: Vec<FileNode>
    }
}

impl FileNode {
    pub fn list_all_files(&self, prefix: String) {
        if prefix == "" {    // 根目录
            println!("path                type                size                todo");
            println!("----------------------------------------------------------------");
        }

        match self {
            FileNode::File { name } => {
                let full_path = prefix + name;
                println!("{:<20}file", full_path);
            },
            FileNode::Directory { name, files } => {
                let full_path = prefix.clone() + name;
                println!("{:<20}directory", full_path);
                for file in files {
                    if prefix == "" {    // 根目录
                        file.list_all_files(String::from("/"));
                    } else {
                        file.list_all_files(format!("{}{}/", prefix, name));
                    }
                }
            }
        }
    }
}