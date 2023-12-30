pub mod virtual_disk;

use std::str;
use core::panic;
use ansi_rgb::Foreground;
use serde::{Deserialize, Serialize};
use std::{fmt, vec::Vec, string::String, usize};
use virtual_disk::{FatStatus, VirtualDisk, BLOCK_SIZE, BLOCK_COUNT};


#[derive(Serialize, Deserialize)]
pub struct DiskInfo {
    pub virtual_disk: VirtualDisk,
    pub cur_directory: Directory,
}


impl DiskInfo {
    /// 初始化新磁盘，返回DiskManager对象。若输入None，则自动创建默认配置。
    pub fn new(root_dir: Option<Directory>) -> DiskInfo {
        pinfo();
        println!("Creating new disk...");
        // 生成虚拟磁盘
        let mut disk = VirtualDisk::new();
        {
            // 放置第一个根目录
            let dir_data: Vec<u8> = bincode::serialize(&root_dir).unwrap();
            disk.insert_data_by_offset(dir_data.as_slice(), 0);
        }
        disk.fat[0] = FatStatus::EOF;

        DiskInfo {
            virtual_disk: disk,
            cur_directory: match root_dir {
                // 默认根目录配置
                None => Directory {
                    name: String::from("root"),
                    files: vec![
                        Fcb {
                            name: String::from(".."),
                            file_type: FileType::Directory,
                            first_cluster: 0,
                            length: 0,
                        },
                        Fcb {
                            name: String::from("."),
                            file_type: FileType::Directory,
                            first_cluster: 0,
                            length: 0,
                        },
                    ],
                },
                Some(dir) => dir,
            },
        }
    }

    // 遍历查找第一个空闲块的块号
    // TODO 有优化的空间
    pub fn find_next_empty_fat(&self) -> Option<usize> {
        let mut res: Option<usize> = None;
        for i in 0..(self.virtual_disk.fat.len() - 1) {
            if let FatStatus::UnUsed = self.virtual_disk.fat[i] {
                res = Some(i);
                break;
            }
        }
        res
    }

    // 查询是否有指定数量的空闲块，如果有在FAT表中修改相关值，然后返回块号数组
    pub fn allocate_free_space_on_fat(
        &mut self,
        clusters_needed: usize,
    ) -> Result<Vec<usize>, &'static str> {
        pinfo();
        println!("Allocating new space...");

        let mut clusters: Vec<usize> = Vec::with_capacity(clusters_needed);
        for i in 0..clusters_needed {
            // 找到一个空闲块
            clusters.push(match self.find_next_empty_fat() {
                Some(cluster) => cluster,
                _ => return Err("[ERROR]\tCannot find a NotUsed FatItem!"),
            });
            
            let cur_cluster: usize = clusters[i];

            // 对磁盘写入数据
            pdebug();
            println!("Found new empty cluster: {}", cur_cluster);
            if i != 0 {
                // 从第二块开始，将上一块的FAT值修改为当前块
                self.virtual_disk.fat[clusters[i - 1]] = FatStatus::ClusterNo(cur_cluster);
            }
            // 每次都将当前块作为最后一块，防止出现没有空闲块提前退出的情况
            self.virtual_disk.fat[cur_cluster] = FatStatus::EOF;
        }

        Ok(clusters)
    }

    // 获取以first_cluster为开头在FAT中所关联的所有文件块
    fn get_file_clusters(&self, first_cluster: usize) -> Result<Vec<usize>, String> {
        pinfo();
        println!("Searching file clusters...");
        let mut clusters: Vec<usize> = Vec::new();
        let mut cur_cluster: usize = first_cluster;

        // 第一块
        clusters.push(first_cluster);

        // 循环读出之后所有块
        loop {
            match self.virtual_disk.fat[cur_cluster] {
                FatStatus::ClusterNo(cluster) => {
                    pdebug();
                    println!("Found next cluster: {}.", cluster);
                    clusters.push(cluster);
                    cur_cluster = cluster;
                }
                FatStatus::EOF => {
                    pdebug();
                    println!("Found EoF cluster: {}.", cur_cluster);
                    break Ok(clusters);
                }
                FatStatus::UnUsed => {
                    break Err(format!(
                        "[ERROR]\tBad cluster detected at {}!",
                        cur_cluster
                    ))
                }
            }
        }
    }

    /// 释放从first_cluster开始已经被分配的块
    fn delete_space_on_fat(&mut self, first_cluster: usize) -> Result<Vec<usize>, String> {
        pinfo();
        println!("Deleting Fat space...");
        let clusters_result: Result<Vec<usize>, String> = self.get_file_clusters(first_cluster);
        let clusters: Vec<usize> = clusters_result.clone().unwrap();
        for cluster in clusters {
            self.virtual_disk.fat[cluster] = FatStatus::UnUsed;
        }

        clusters_result
    }

    // 计算写入文件需要的块数量——针对EoF
    // 返回（`bool`: 是否需要插入EoF，`usize`: 需要的总块数）
    fn calc_clusters_needed_with_eof(length: usize) -> (bool, usize) {
        // 需要的块数
        let mut clusters_needed: f32 = length as f32 / BLOCK_SIZE as f32;

        // 需要的块数为整数不需要写入结束标志，否则需要写入结束标志
        let insert_eof: bool = if (clusters_needed - clusters_needed as usize as f32) < 0.0000000001 {
            false
        } else {
            // 向上取整
            clusters_needed = clusters_needed.ceil();
            true
        };
        let clusters_needed: usize = clusters_needed as usize;

        (insert_eof, clusters_needed)
    }

    // 写入的数据到硬盘，返回first_cluster
    pub fn write_data_to_disk(&mut self, data: &[u8]) -> usize {
        pinfo();
        println!("Writing data to disk...");

        let (insert_eof, clusters_needed) = DiskInfo::calc_clusters_needed_with_eof(data.len());

        let clusters: Vec<usize> = self.allocate_free_space_on_fat(clusters_needed).unwrap();

        self.virtual_disk.write_data_by_clusters_with_eof(data, clusters.as_slice(), insert_eof);

        pdebug();
        println!("Writing finished. Returned clusters: {:?}", clusters);

        clusters[0]
    }

    // 在当前目录中新建目录，并且写入磁盘
    pub fn new_directory_to_disk(&mut self, name: &str) -> Result<(), &'static str> {
        // 新文件夹写入磁盘块
        pinfo();
        println!("Creating dir: {}.", name);
        pdebug();
        println!("Trying to write to disk...");

        if let Some(_fcb) = self.cur_directory.get_fcb_by_name(name) {
            return Err("[ERROR]\tThere's already a directory with a same name!");
        }

        // Directory对象是目录的数据，每个数据项是一个Fcb
        let mut new_directory: Directory = Directory::new(name);
        // 添加父目录，用于cd切换到父目录
        new_directory.files.push(Fcb {
            name: String::from(".."),
            file_type: FileType::Directory,
            first_cluster: self.cur_directory.files[1].first_cluster,
            length: 0,
        });
        // TODO: 为什么要加入自己？
        new_directory.files.push(Fcb {
            name: String::from("."),
            file_type: FileType::Directory,
            first_cluster: self.find_next_empty_fat().unwrap(),
            length: 0,
        });

        let bin_dir: Vec<u8> = bincode::serialize(&new_directory).unwrap();

        pdebug();
        println!("Dir bytes: {:?}", bin_dir);
        // 将新建的目录写入到硬盘
        let first_block: usize = self.write_data_to_disk(&bin_dir);

        pdebug();
        println!("Trying to add dir to current dir...");

        // 在当前目录添加新目录
        self.cur_directory.files.push(Fcb {
            name: String::from(name),
            file_type: FileType::Directory,
            first_cluster: first_block,
            length: 0,
        });
        pdebug();
        println!("Created dir {}.", name);

        // 这里并没有立即更新当前目录到硬盘，而是等切换目录或退出时再保存
        // 因为可能创建多个目录，如果每创建一个就更新一次效率会比较低
        // 但也会有新的问题，比如没有正常退出（如断电）会导致数据丢失
        Ok(())
    }

    // 根据首块块号，读出所有数据
    fn get_data_by_first_cluster(&self, first_cluster: usize) -> Vec<u8> {
        pdebug();
        println!("Getting data from disk by clusters...");

        let clusters: Vec<usize> = self.get_file_clusters(first_cluster).unwrap();
        let data: Vec<u8> = self
            .virtual_disk
            .read_data_by_clusters_without_eof(clusters.as_slice());

        pdebug();
        println!("Data read: {:?}", &data);

        data
    }

    // 通过FCB块找到目录数据
    fn get_directory_by_fcb(&self, dir_fcb: &Fcb) -> Directory {
        pinfo();
        println!("Getting dir by FCB...\n\tFCB: {:?}", dir_fcb);
        match dir_fcb.file_type {
            FileType::Directory => {
                let data_dir = self.get_data_by_first_cluster(dir_fcb.first_cluster);
                pdebug();
                println!("Trying to deserialize data read from disk...");
                let dir: Directory = bincode::deserialize(data_dir.as_slice()).unwrap();
                pdebug();
                println!("Getting dir finished.");
                dir
            }
            _ => panic!("[ERROR]\tGet Directory recieved a non-Directory FCB!"),
        }
    }

    // 通过FCB块找到文件数据
    fn get_file_by_fcb(&self, fcb: &Fcb) -> Vec<u8> {
        pinfo();
        println!("Getting file data by FCB...\n\tFCB: {:?}", fcb);
        match fcb.file_type {
            FileType::File => self.get_data_by_first_cluster(fcb.first_cluster),
            _ => panic!("[ERROR]\tGet File recieved a non-File FCB!"),
        }
    }


    // 在当前目录新建文件并写入数据
    pub fn create_file_with_data(&mut self, name: &str, data: &[u8]) {
        pinfo();
        println!("Creating new file in current dir...");
        // 写入数据
        let first_cluster = self.write_data_to_disk(data);
        // 创建新FCB并插入当前目录中
        let fcb: Fcb = Fcb {
            name: String::from(name),
            file_type: FileType::File,
            first_cluster,
            length: data.len(),
        };
        self.cur_directory.files.push(fcb);
    }

    // 通过文件名读取文件
    pub fn read_file_by_name(&self, name: &str) -> Vec<u8> {
        let (_index, fcb) = self.cur_directory.get_fcb_by_name(name).unwrap();
        self.get_file_by_fcb(fcb)
    }

    // 通过文件名删除文件
    pub fn delete_file_by_name(&mut self, name: &str) -> Result<(), String> {
        let index: usize = self.cur_directory.get_index_by_name(name).unwrap();
        // 从dir中先删除fcb，如果删除失败再还回来
        pdebug();
        println!("Trying to delete file in dir file list...");
        let fcb: Fcb = self.cur_directory.files.remove(index);
        let res: Result<(), String> = self.delete_file_by_fcb_with_index(&fcb, None);

        if res.is_err() {
            self.cur_directory.files.push(fcb);
        }

        res
    }

    // 首先要清除文件分配表中占用的块，数据区可以不清零，然后还要从父目录中删除对应的FCB
    fn delete_file_by_fcb_with_index(
        &mut self,
        fcb: &Fcb,
        index: Option<usize>,
    ) -> Result<(), String> {
        if let FileType::Directory = fcb.file_type {
            let dir: Directory = self.get_directory_by_fcb(fcb);
            if dir.files.len() > 2 {
                return Err(String::from("[ERROR]\tThe Directory is not empty!"));
            }
        }
        pdebug();
        println!(
            "Trying to set all NotUsed clutster of file '{}' on FAT...",
            fcb.name
        );
        // 直接返回删除文件的结果
        if let Err(err) = self.delete_space_on_fat(fcb.first_cluster) {
            return Err(err);
        }
        // 若给定index非None，则删除目录下的FCB条目
        if let Some(i) = index {
            self.cur_directory.files.remove(i);
        }

        Ok(())
    }

    // 切换到指定目录
    pub fn change_current_directory(&mut self, name: &str) {
        // 先保存当前目录数据到硬盘
        let dir_cloned: Directory = self.cur_directory.clone();
        self.save_directory_to_disk(&dir_cloned);
        // 通过name获取要切换到的目录fcb
        let (_index, dir_fcb) = self.cur_directory.get_fcb_by_name(name).unwrap();

        let dir: Directory = self.get_directory_by_fcb(dir_fcb);
        self.cur_directory = dir;
    }

    // 保存当前目录数据到硬盘，返回第一个块号——更改被保存，原目录文件将在磁盘上被覆盖
    fn save_directory_to_disk(&mut self, dir: &Directory) -> usize {
        pdebug();
        println!("Trying to saving dir...");
        let data = bincode::serialize(dir).unwrap();
        let (insert_eof, clusters_needed) = DiskInfo::calc_clusters_needed_with_eof(data.len());
        // 删除原先的块
        self.delete_space_on_fat(self.cur_directory.files[1].first_cluster).unwrap();
        // 分配新的块
        let reallocated_clusters = self.allocate_free_space_on_fat(clusters_needed).unwrap();
        self.virtual_disk.write_data_by_clusters_with_eof(
            data.as_slice(),
            reallocated_clusters.as_slice(),
            insert_eof,
        );

        reallocated_clusters[0]
    }

    // 文件改名
    // 目录改名要复杂一些，这里没实现
    pub fn rename_file_by_name(&mut self, old: &str, new: &str) {
        let (index, fcb) = self.cur_directory.get_fcb_by_name(old).unwrap();
        let new_fcb: Fcb = Fcb {
            name: String::from(new),
            ..fcb.to_owned()
        };
        self.cur_directory.files[index] = new_fcb;
    }

    // 移动文件
    pub fn movie_file_by_name(&mut self, file_name: &str, path: &str) {
        let index = self.cur_directory.get_index_by_name(file_name).unwrap();
        // 从当前目录中删除fcb
        let fcb: Fcb = self.cur_directory.files.remove(index);
        self.save_directory_to_disk(&self.cur_directory.clone());
        
        let dir_names: Vec<&str> = path.split("/").collect();
        let mut cur_directory = self.cur_directory.clone();
        for dir_name in dir_names {
            if dir_name == "" {
                continue;
            }
            let (_, dir_fcb) = cur_directory.get_fcb_by_name(dir_name).unwrap();
            cur_directory = self.get_directory_by_fcb(dir_fcb);
        }
        cur_directory.files.push(fcb);
        let data: Vec<u8> = bincode::serialize(&cur_directory).unwrap();
        let (insert_eof, clusters_needed) = DiskInfo::calc_clusters_needed_with_eof(data.len());
        // 删除原先的块
        self.delete_space_on_fat(cur_directory.files[1].first_cluster).unwrap();
        // 分配新的块
        let reallocated_clusters: Vec<usize> = self.allocate_free_space_on_fat(clusters_needed).unwrap();
        self.virtual_disk.write_data_by_clusters_with_eof(
            data.as_slice(),
            reallocated_clusters.as_slice(),
            insert_eof,
        );
    }

    // 获取部分磁盘信息
    // 返回 磁盘总大小/Byte，已分配块数量、未分配块的数量
    pub fn get_disk_info(&self) -> (usize, usize, usize) {
        let disk_size: usize = BLOCK_SIZE * BLOCK_COUNT;
        let mut num_used: usize = 0usize;
        let mut num_not_used: usize = 0usize;

        for fat_item in &self.virtual_disk.fat {
            match fat_item {
                FatStatus::ClusterNo(_no) => num_used += 1,
                FatStatus::EOF => num_used += 1,
                FatStatus::UnUsed => num_not_used += 1,
            }
        }

        (disk_size, num_used, num_not_used)
    }

    // FCB的移动
    // 这个也没用上
    pub fn move_fcb_between_dirs_by_name(&mut self, name: &str, des_dir: &mut Directory) {
        let fcb = self
            .cur_directory
            .files
            .remove(self.cur_directory.get_index_by_name(name).unwrap());
        des_dir.files.push(fcb);
    }

    // 复制文件
    pub fn copy_file_by_name(&mut self, raw_name: &str, new_name: &str) -> bool {
        let (_, fcb) = self.cur_directory.get_fcb_by_name(raw_name).unwrap();
        let data: Vec<u8> = self.get_file_by_fcb(fcb);
        self.create_file_with_data(new_name, &data);
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileType {
    File,
    Directory,
}
impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileType::Directory => write!(f, "Directory"),
            FileType::File => write!(f, "File"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fcb {
    name: String,         // 文件名
    file_type: FileType,  // 文件类型
    first_cluster: usize, // 起始块号
    length: usize,        // 文件大小
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Directory {
    name: String,
    files: Vec<Fcb>,
}
impl Directory {
    fn new(name: &str) -> Directory {
        Directory {
            name: String::from(name),
            files: Vec::with_capacity(2),
        }
    }

    // 通过文件名获取文件在files中的索引和文件FCB
    fn get_fcb_by_name(&self, name: &str) -> Option<(usize, &Fcb)> {
        let mut res: Option<(usize, &Fcb)> = None;
        for i in 0..self.files.len() {
            if self.files[i].name.as_str() == name {
                res = Some((i, &self.files[i]));
                break;
            }
        }

        res
    }

    // 通过文件名获取文件在files中的索引
    fn get_index_by_name(&self, name: &str) -> Option<usize> {
        let mut res: Option<usize> = None;
        for i in 0..self.files.len() {
            if self.files[i].name.as_str() == name {
                res = Some(i);
                break;
            }
        }

        res
    }
}

impl fmt::Display for Directory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // 仅将 self 的第一个元素写入到给定的输出流 `f`。返回 `fmt:Result`，此
        // 结果表明操作成功或失败。注意 `write!` 的用法和 `println!` 很相似。
        writeln!(f, "Directroy '{}' Files:", self.name)?;
        for file in &self.files {
            writeln!(
                f,
                "{}\t\t{}\t\tLength: {}",
                file.name, file.file_type, file.length
            )?;
        }

        fmt::Result::Ok(())
    }
}

pub fn pdebug() {
    print!("{}", "[DEBUG]\t".fg(ansi_rgb::magenta()));
}

pub fn pinfo() {
    print!("{}", "[INFO]\t".fg(ansi_rgb::cyan_blue()));
}