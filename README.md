# rust_file_system
使用Rust语言写的一个虚拟文件系统，使用一个文件来模拟磁盘

## 使用说明
使用命令行交互界面，支持以下命令：
- `cd <dirname>`: 更改当前目录
- `ls` : 查看当前目录下的所有文件
- `cat <filename>`: 查看文件内容
- `mkdir <dirname>`: 新建目录
- `cp <filename> <new_filename>` : 复制文件
- `rename <raw_name> <new_name>` : 重命名文件
- `rm <filename>`: 删除文件
- `mv <filename> <path>` : 移动文件
- `save` : 保存文件系统
- `diskinfo` : 查看磁盘使用情况
- `exit` : 退出程序

