//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, TASK_MANAGER},
    timer::get_time_us,
};
use crate::syscall::TASK_INFOLIST;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

impl TaskInfo {
    pub fn new() -> Self {
        TaskInfo {
            status: TaskStatus::Ready,
            syscall_times: [0; MAX_SYSCALL_NUM],
            time: 0,
        }
    }

    /// 改变任务状态
    pub fn change_status(&mut self, cur: TaskStatus) {
        self.status = cur;
    }

    /// 改变任务系统调用次数
    /// 需要传入当前系统调用id
    pub fn add_syscall_time(&mut self, syscall_id: usize) {
        self.syscall_times[syscall_id] += 1;
    }

    /// 改变任务距第一次调用的时间
    /// 需要传入任务id和当前时间
    pub fn change_time(&mut self, cur_time: usize, id: usize) {
        let time_list = TASK_INFOLIST.task_init_times.access();
        self.time = cur_time - time_list[id];
    }
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    // 注意不能这样写：善用getter、setter
    // let task_id = TASK_MANAGER.inner.exclusive_access();
    let task_id = TASK_MANAGER.get_current_id();
    // 获取不可变引用
    let task_infos = TASK_INFOLIST.task_infos.access();
    unsafe {
        (*_ti).status = task_infos[task_id].status;
        (*_ti).syscall_times = task_infos[task_id].syscall_times;
        (*_ti).time = task_infos[task_id].time;
    }
    0
}
