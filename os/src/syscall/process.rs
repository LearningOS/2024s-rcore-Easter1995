//! Process management syscalls
use core::mem::{size_of, transmute};

use crate::{
    config::MAX_SYSCALL_NUM, mm::translated_byte_buffer, task::{
        change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus
    }, timer::get_time_us
};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    // 尝试将按应用的虚地址指向的缓冲区转换为一组按内核虚地址指向的字节数组切片构成的向量
    let mut ts_buffer = translated_byte_buffer(current_user_token(), _ts as *const u8, size_of::<TimeVal>());
    // 计算出正确的时间
    let us = get_time_us();
    let time: TimeVal = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    // What if [`TimeVal`] is splitted by two pages ?
    // sec和usec都被转为长度为8以字节为单位的数组
    let time_bytes: [u8; 16] = unsafe { transmute(time.clone()) };
    // 判断是否跨页
    if ts_buffer[0].len() >= 16 {
        // 第一页就可以存下time
        let page_ptr = ts_buffer[0].as_mut_ptr() as *mut TimeVal;
        unsafe {
            (*page_ptr) = time;
        }
    } else {
        let available_len = ts_buffer[0].len();
        ts_buffer[0].copy_from_slice(&time_bytes[..available_len]);
        ts_buffer[1].copy_from_slice(&time_bytes[available_len..]);
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    -1
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    -1
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
