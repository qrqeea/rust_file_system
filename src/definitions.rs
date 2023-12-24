
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
    pub fn print_tree(&self) {
        match self {
            FileNode::File { name } => {
                println!("{}", name);
            },
            FileNode::Directory { name, files } => {
                println!("{}", name);
                for file in files {
                    file.print_tree()
                }
            }
        }
    }
}