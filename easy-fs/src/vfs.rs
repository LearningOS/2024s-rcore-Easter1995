use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    pub fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        // 计算disk_inode上存了多少个文件/索引
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        // 新建一个文件索引
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            // 因此读取目录项到dirent中，确保目录项的大小都是DIRENT_SZ
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            // 如果读到name==name的dirent，说明找到了这个inode，返回inode的编号
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode_id by name
    pub fn find_inode_id_by_name(&self, name: &str) -> Option<u32> {
        // assert it is a directory
        // 仿照find的写法，但是返回inode_id而不是inode
        assert!(self.is_dir());
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode)
        })
    }
    /// 通过磁盘上的位置找到inode并取得其id
    pub fn find_inode_id_by_pos(&self) -> u32 {
        self.fs.lock().get_inode_id(self.block_id, self.block_offset)
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        let inode = Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        ));
        // 硬连接数记得++
        inode.modify_disk_inode(DiskInode::hard_link_add);
        Some(inode)
        // release efs lock automatically by compiler
    }
    /// 相当于新建一个目录项，但是此目录项指向的inode跟old_name那项的inode一样
    pub fn create_link(&self, new_name: &str, old_name: &str) -> isize {
        let mut fs = self.fs.lock();
        let op1 = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // 是否链接同名文件?
            self.find_inode_id(new_name, root_inode)
        };
        if self.read_disk_inode(op1).is_some() {
            return -1;
        }

        if let Some(inode_id) = self.find_inode_id_by_name(old_name) {
            // 将新的名字跟之前的inode关联起来形成目录项插入目录，使得之后可以靠这个名字获取索引从而访问磁盘文件
            self.modify_disk_inode(|root_inode| {
                // append file in the dirent
                let file_count = (root_inode.size as usize) / DIRENT_SZ;
                let new_size = (file_count + 1) * DIRENT_SZ;
                // increase size
                self.increase_size(new_size as u32, root_inode, &mut fs);
                // write dirent
                let dirent = DirEntry::new(new_name, inode_id);
                root_inode.write_at(
                    file_count * DIRENT_SZ,
                    dirent.as_bytes(),
                    &self.block_device,
                );
            });
            // 如果成功创建连接，节点硬连接数量++
            // 根据inode_id修改DiskInode
            let inode = self.find(old_name).unwrap();
            inode.modify_disk_inode(DiskInode::hard_link_add);

            block_cache_sync_all();
            return 0;
            // release efs lock automatically by compiler
        } 
        // 旧的文件名就不存在
        -1
    }
    /// 删除硬连接
    pub fn del_link(&self, name: &str) -> isize {
        // 确保该名字有对应的inode
        if let Some(inode) = self.find(name) {
            // 硬连接数-1
            inode.modify_disk_inode(DiskInode::hard_link_del);
            // 只有一个硬连接
            if inode.read_disk_inode(DiskInode::get_hard_link_num) == 1 {
                self.clear();
            }
            // 否则清除掉对应的目录项
            self.modify_disk_inode(|root_inode| {
                // assert it is a directory
                assert!(root_inode.is_dir());
             
                let file_count = (root_inode.size as usize) / DIRENT_SZ;
                let mut dirent = DirEntry::empty();
                // 在目录表中删除名为name的那一项
                for i in 0..file_count {
                    assert_eq!(
                        root_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                        DIRENT_SZ,
                    );
                    if dirent.name() == name {
                        let new_dirent = DirEntry::empty();
                        root_inode.write_at(DIRENT_SZ * i, new_dirent.as_bytes(), &self.block_device);
                        break;
                    }
                }
            });   
        }
        block_cache_sync_all();
        0
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    /// Get inode type
    pub fn is_dir(&self) -> bool {
        self.read_disk_inode(|d| d.is_dir())
    }
    /// Get inode id
    pub fn get_block_id(&self) -> usize {
        self.block_id
    }
    /// Get hard link's num
    pub fn get_hard_link_num(&self) -> u32 {
        self.read_disk_inode(DiskInode::get_hard_link_num)
    }
}