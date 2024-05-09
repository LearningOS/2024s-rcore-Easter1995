//! Process management syscalls
//!
use alloc::sync::Arc;
use core::mem::{size_of, transmute};
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str, translated_byte_buffer, VirtAddr, MapPermission},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus, TaskControlBlock
    },
    timer::get_time_us,
    syscall::TASK_INFOLIST, 
};

#[repr(C)]
#[derive(Debug)]
/// 
pub struct TimeVal {
    /// 毫秒数
    pub sec: usize,
    /// 秒数
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
    /// 初始化函数
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
        let initial_time = time_list.get(&id).unwrap();
        self.time = cur_time - initial_time;
    }
}
/// 退出
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}
/// 转让cpu
pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}
/// 获取pid
pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}
/// 创建新进程
pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}
/// 切换到指定任务
pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // 尝试将按应用的虚地址指向的缓冲区转换为一组按内核虚地址指向的字节数组切片构成的向量
    let mut ts_buffer = translated_byte_buffer(current_user_token(), _ts as *const u8, size_of::<TimeVal>());
    // 计算出正确的时间
    let us = get_time_us();
    let time: TimeVal = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    // What if [`TimeVal`] is splitted by two pages ?
    // 判断是否跨页
    if ts_buffer[0].len() >= 16 {
        // 第一页就可以存下time
        let page_ptr = ts_buffer[0].as_mut_ptr() as *mut TimeVal;
        unsafe {
            (*page_ptr) = time;
        }
    } else {
        // 将已经包装好的time转换为以字节为单位的数组
        let time_bytes: [u8; 16] = unsafe { transmute(time) };
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
    trace!(
        "kernel:pid[{}] sys_task_info NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let mut ti_buffer = translated_byte_buffer(current_user_token(), _ti as *const u8, size_of::<TaskInfo>());
    let task_id = &current_task().unwrap().getpid();
    // 获取不可变引用
    let task_infos = TASK_INFOLIST.task_infos.access();
    // 创建要返回的TaskInfo
    let info = TaskInfo {
        status: task_infos.get(task_id).unwrap().status,
        syscall_times: task_infos.get(task_id).unwrap().syscall_times,
        time: task_infos.get(task_id).unwrap().time
    };
    // What if [`TimeVal`] is splitted by two pages ?
    // 判断是否跨页
    if ti_buffer[0].len() >= size_of::<TaskInfo>() {
        // 第一页就可以存下info
        let page_ptr = ti_buffer[0].as_mut_ptr() as *mut TaskInfo;
        unsafe {
            (*page_ptr) = info;
        }
    } else {
        // 将已经包装好的info转换为以字节为单位的数组
        let available_len = ti_buffer[0].len();
        let info_bytes: [u8; size_of::<TaskInfo>()] = unsafe { transmute(info) };
        ti_buffer[0].copy_from_slice(&info_bytes[..available_len]);
        ti_buffer[1].copy_from_slice(&info_bytes[available_len..]);   
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_mmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // start 需要映射的虚存起始地址，要求按页对齐
    // start 没有按页大小对齐
    if _start % PAGE_SIZE != 0 {
        return  -1;
    }
    // port & !0x7 != 0 (port 其余位必须为0)
    // port & 0x7 = 0 (这样的内存无意义)
    if (_port & !0x7 != 0) || (_port & 0x7 == 0) {
        return -1;
    }
    // [start, start + len) 中存在已经被映射的页
    let start_vpn = VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    // let task_control_block = TASK_MANAGER.get_task_control_block(TASK_MANAGER.get_current_id());
    // 现在可以直接获取任务控制块
    // 获取中间值
    let task_control_block = current_task().unwrap();
    // 获取inner
    let mut task_control_block_inner = task_control_block.inner_exclusive_access();
    if task_control_block_inner.is_overlap(start_vpn, end_vpn) {
        return -1;
    }
    // 参数检查结束，开始分配空间
    // U模式有效    
    let per = MapPermission::from_bits((_port as u8) << 1).unwrap() | MapPermission::U;
    task_control_block_inner.insert_frame(_start, _start + _len, per);
    drop(task_control_block_inner);
    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_munmap NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    // start 需要映射的虚存起始地址，要求按页对齐
    // start 没有按页大小对齐
    if _start % PAGE_SIZE != 0 {
        return  -1;
    }
    // [start, start + len) 中存在未被映射的虚存
    let start_vpn = VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    // 获取中间值
    let task_control_block = current_task().unwrap();
    // 获取inner
    let mut task_control_block_inner = task_control_block.inner_exclusive_access();
    // 不存在未被映射的虚存
    if task_control_block_inner.memory_set.is_all_exist(start_vpn, end_vpn) {
        // 这片区域的虚存都存在，取消映射
        task_control_block_inner.memory_set.mem_set_unmap(start_vpn, end_vpn);
        return 0;
    }
    -1
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, _path);

    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let current_task = current_task().unwrap();
        // 但提醒读者 spawn 不必像 fork 一样复制父进程的地址空间
        // let new_task = current_task.fork();
        // let new_pid = new_task.getpid();
        // new_task.exec(app);

        // 手动创建一个任务
        // 从索引节点获取数据
        let all_data = app_inode.read_all();
        // 新建任务控制块
        let new_task = Arc::new(TaskControlBlock::new(all_data.as_slice()));
        let new_pid = new_task.getpid();
        // 添加到TASK_MANAGER
        add_task(new_task.clone());
        // 添加新进程到现在任务的子进程
        let mut parent_inner = current_task.inner_exclusive_access();
        parent_inner.children.push(new_task.clone());
        // 返回pid
        return new_pid as isize;
    }
    -1
}

/// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio <= 1 {
        return -1;
    }
    let current_task = current_task().unwrap();
    let mut cur_pri = current_task.priority.exclusive_access();
    *cur_pri = _prio;
    _prio
}
