//!Implementation of [`TaskManager`]

use super::TaskControlBlock;
use crate::config::BIG_STRIDE;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Update Stride by Index
    pub fn update_stride_by_index(&mut self, index: usize) {
        let task = self.ready_queue.get(index).unwrap();
        task.update_stride();
        // // 溢出了
        // if let Some(min_stride_index) = self.get_min_stride_index() {
        //     let min_stride = *self
        //         .ready_queue
        //         .get(min_stride_index)
        //         .unwrap()
        //         .stride
        //         .access();
        //     for task in self.ready_queue.iter_mut() {
        //         task.update_stride_when_overflow(min_stride);
        //     }
        // } else {
        //     task.update_stride_when_overflow(0);
        // }
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        // self.ready_queue.pop_front() 取消先进先出的算法
        // stride算法
        let mut min_index = 0;
        let mut min_stride = BIG_STRIDE;
        if self.ready_queue.is_empty() {
            return None;
        }
        // 暴力枚举
        for i in 0..self.ready_queue.len() {
            let task = self.ready_queue.get(i).unwrap();
            let stride = *task.stride.access();
            if stride <= min_stride {
                min_index = i;
                min_stride = stride;
            }
        }
        let task = self.ready_queue.get(min_index).unwrap();
        task.update_stride();
        self.ready_queue.remove(min_index)
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
