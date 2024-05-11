//! File and filesystem-related syscalls
use core::mem::{size_of, transmute};
use crate::fs::{open_file, OpenFlags, Stat};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
/// 功能：获取文件状态
/// fd: 文件描述符
/// st: 文件状态结构体
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // 根据文件描述符取得文件
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    // 如果文件不存在，返回-1
    if _fd >= inner.fd_table.len() {
        return -1;
    }
    // 根据文件描述符获取文件
    if let Some(file) = &inner.fd_table[_fd] {
        let file = file.clone();
        // 获取file过后就可以drop
        drop(inner);
        // 获取文件状态
        let stat = file.stat();
        // 获取_st的可写缓存
        let mut st_buffer = translated_byte_buffer(token, _st as *const u8, size_of::<Stat>());
        if st_buffer[0].len() >= size_of::<Stat>() {
            let page_ptr = st_buffer[0].as_mut_ptr() as *mut Stat;
            unsafe {
                (*page_ptr) = stat
            }
        } else {
            let available_len = st_buffer[0].len();
            let stat_bytes: [u8; size_of::<Stat>()] = unsafe {
                transmute(stat)
            };
            st_buffer[0].copy_from_slice(&stat_bytes[..available_len]);
            st_buffer[1].copy_from_slice(&stat_bytes[available_len..]);
        }
        return 0;
    }
    -1
}

/// YOUR JOB: Implement linkat.
/// 将不同的名字存在目录表上，但是指向同一个inode
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_name = translated_str(token, _old_name);
    let new_name = translated_str(token, _new_name);
    let root_inode = crate::fs::ROOT_INODE.clone();
    if let Some(_inode) = root_inode.find(old_name.as_str()) {
        return root_inode.create_link(new_name.as_str(), root_inode.find_inode_id_by_name(old_name.as_str()).unwrap());
    }
    -1
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let name = translated_str(token, _name);
    let root_inode = crate::fs::ROOT_INODE.clone();
    if let Some(inode) = root_inode.find(name.as_str()) {
        return inode.del_link(name.as_str(), &root_inode);
    }
    -1
}