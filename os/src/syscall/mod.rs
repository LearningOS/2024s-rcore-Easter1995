//! Implementation of syscalls
//!
//! The single entry point to all system calls, [`syscall()`], is called
//! whenever userspace wishes to perform a system call using the `ecall`
//! instruction. In this case, the processor raises an 'Environment call from
//! U-mode' exception, which is handled as one of the cases in
//! [`crate::trap::trap_handler`].
//!
//! For clarity, each single syscall is implemented as its own function, named
//! `sys_` then the name of the syscall. You can find functions like this in
//! submodules, and you should also implement syscalls this way.
const SYSCALL_WRITE: usize = 64;
/// exit syscall
const SYSCALL_EXIT: usize = 93;
/// yield syscall
const SYSCALL_YIELD: usize = 124;
/// gettime syscall
const SYSCALL_GET_TIME: usize = 169;
/// sbrk syscall
const SYSCALL_SBRK: usize = 214;
/// munmap syscall
const SYSCALL_MUNMAP: usize = 215;
/// mmap syscall
const SYSCALL_MMAP: usize = 222;
/// taskinfo syscall
const SYSCALL_TASK_INFO: usize = 410;

mod fs;
mod process;

use alloc::vec::Vec;
use fs::*;
use process::*;
use lazy_static::*;
use crate::{
    sync::UPSafeCell,
    // task::TASK_MANAGER,
    // timer::get_time_ms,
};


/// 所有task的信息
pub struct TaskInfoList {
    /// 任务信息
    /// 有堆空间了所以采用动态数组
    pub task_infos: UPSafeCell<Vec<TaskInfo>>,
    /// 任务第一次被初始化的时间
    pub task_init_times: UPSafeCell<Vec<usize>>,
}

lazy_static! {
    /// 创建全局变量TASK_INFOS实例，包含两个动态数组
    pub static ref TASK_INFOLIST: TaskInfoList = {
        let task_infos_inner: Vec<TaskInfo> = Vec::new();
        let init_times_inner: Vec<usize> = Vec::new();

        TaskInfoList {
            task_infos: unsafe {
                UPSafeCell::new(task_infos_inner)
            },
            task_init_times: unsafe {
                UPSafeCell::new(init_times_inner)
            }
        }
    };
}

/// handle syscall exception with `syscall_id` and other arguments
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    // 初始化task系统调用次数的信息
    // let mut task_infos = TASK_INFOLIST.task_infos.exclusive_access();
    // let current_id = TASK_MANAGER.get_current_id();
    // task_infos[current_id].change_time(get_time_ms(), current_id);

    match syscall_id {
        // SYSCALL_WRITE => {
        //     task_infos[current_id].add_syscall_time(SYSCALL_WRITE);
        //     drop(task_infos);
        //     sys_write(args[0], args[1] as *const u8, args[2])
        // },
        // SYSCALL_EXIT => {
        //     task_infos[current_id].add_syscall_time(SYSCALL_EXIT);
        //     drop(task_infos);
        //     sys_exit(args[0] as i32)
        // },
        // SYSCALL_YIELD => {
        //     task_infos[current_id].add_syscall_time(SYSCALL_YIELD);
        //     drop(task_infos);
        //     sys_yield()
        // }, 
        // SYSCALL_GET_TIME => {
        //     task_infos[current_id].add_syscall_time(SYSCALL_GET_TIME);
        //     drop(task_infos);
        //     sys_get_time(args[0] as *mut TimeVal, args[1])  
        // }, 
        // SYSCALL_TASK_INFO => {
        //     task_infos[current_id].add_syscall_time(SYSCALL_TASK_INFO);
        //     drop(task_infos);
        //     sys_task_info(args[0] as *mut TaskInfo)
        // },
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        SYSCALL_YIELD => sys_yield(),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *mut TimeVal, args[1]),
        SYSCALL_TASK_INFO => sys_task_info(args[0] as *mut TaskInfo),
        SYSCALL_MMAP => sys_mmap(args[0], args[1], args[2]),
        SYSCALL_MUNMAP => sys_munmap(args[0], args[1]),
        SYSCALL_SBRK => sys_sbrk(args[0] as i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
