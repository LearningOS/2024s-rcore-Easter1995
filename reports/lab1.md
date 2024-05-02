# **Lab1**

## 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

   > *《你交流的对象说明》*
   >
   > 无交流

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

   > *《你参考的资料说明》*
   >
   > 除实验文档外无其他资料

\3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

\4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 编程作业

### 流程梳理

首先看入口函数rust_main()：

```rust
pub fn rust_main() -> ! {
    clear_bss();
    kernel_log_info();
    heap_alloc::init_heap();
    trap::init();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}
```

- trap::init()：初始化Trap，主要内容就是执行 __alltraps 函数，把当前任务的寄存器信息以Trap上下文的形式保存在内核栈上
- loader::load_apps()：加载程序，主要就是将所有的应用程序复制到规定的内存区域来
- task::run_first_task()：一切准备就绪后，开始执行第一个程序

```rust
/// Run the first task in task list.
///
/// Generally, the first task in task list is an idle task (we call it zero process later).
/// But in ch3, we load apps statically, so the first task is a real app.
fn run_first_task(&self) -> ! {
    let mut inner = self.inner.exclusive_access();
    let task0 = &mut inner.tasks[0];
    task0.task_status = TaskStatus::Running;
    let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
    drop(inner);
    let mut _unused = TaskContext::zero_init();
    // before this, we should drop local variables that must be dropped manually
    unsafe {
        __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
    }
    panic!("unreachable in run_first_task!");
}
```

- 由于这个是TaskManager的方法，因此调用该方法时也顺便把TASK_MANAGER实例也初始化了，也同时初始化了所有的task块，将其上下文压入了各自的内核栈
- 获取任务管理器inner的可变引用，获取第一个应用程序的上下文位置
- 切换到这个位置开始执行程序
    - 换栈
    - 加载寄存器
    - 跳到ra指向的位置

### 思路

- 所有任务的信息：使用一个全局数组来存储，元素为结构体TaskInfo
- 任务使用的系统调用及调用次数：每进入syscall就+1，注意syscall_id必须存在
- 系统调用时刻距离任务第一次被调度时刻的时长：初始化为0，调用时存储开始时间，然后每调用一个syscall就统计一次过了多久
- 任务状态：初始化为Ready，只有在task模块更改任务状态时更新

### 坑

- 注意不能这样写：let task_id = TASK_MANAGER.inner.exclusive_access();
    
    善用getter、setter
    
- 一定要记得手动drop掉使用exclusive_access()获得的可变引用