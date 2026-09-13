#![no_std]
//! The Kairos arm of the flash comparison: the same kernel operations the C
//! arm's `root.c` pins, so a linker can be asked the same question of both.
//!
//! # The root set is the corpus, and that is the whole argument
//!
//! `docs/API-MAP.md` lists 341 FreeRTOS entry points, nearly all still
//! `planned` here. Linking the C kernel against all of them and Kairos
//! against the forty it implements would price a feature gap, not a kernel.
//! The operations below are the ones the conformance corpus exercises — the
//! one place the two kernels are known to do the SAME WORK, eighteen
//! scenarios byte-identical on four architectures. That is the only root set
//! a footprint comparison can honestly use, and it is the set `root.c`
//! names.
//!
//! # One exported function per operation, and why that matters
//!
//! The first version of this probe called every operation from a single
//! function. That is a bad instrument for two reasons, and both showed up in
//! the map: the optimiser inlined most of the kernel into that one function,
//! so **13,314 bytes were attributed to "the probe"** and the per-operation
//! split became meaningless; and one call site is not how a kernel is
//! actually used, so whatever inlining it produced was an artefact of the
//! harness rather than a property of the kernel.
//!
//! So each operation gets its own `extern "C"` entry point, exactly as the C
//! arm has one exported symbol per operation. The linker is then asked the
//! same question of both — keep what these entry points need, discard the
//! rest — and every byte lands in a section named after the operation that
//! caused it.
//!
//! The kernel arrives by pointer rather than being built here, so no entry
//! point is charged for a construction the C arm does not pay for either.

use rusty_rtos_core::config::PosixDemoConfig;
use rusty_rtos_core::hooks::NoTickHook;
use rusty_rtos_core::trace::NoTrace;
use rusty_rtos_core::handle::TaskHandle;
use rusty_rtos_kernel_core::kernel::{Kernel, NotifyAction};
use rusty_rtos_kernel_core::{items_for, lists_for};
use rusty_rtos_port_riscv::RiscvPort;

const PRIOS: u8 = 5;
const TASKS: usize = 8;
const QUEUES: usize = 8;
const SLOTS: usize = 64;
const BUFFERS: usize = 4;
const BYTES: usize = 1024;
const TIMERS: usize = 16;
const GROUPS: usize = 2;

/// The real RISC-V port, not the sim one, because the C arm links `port.c`
/// and `portASM.S`. A kernel over a real port against a kernel over a
/// simulation would not be a comparison.
pub type K = Kernel<
    PosixDemoConfig,
    RiscvPort,
    NoTrace,
    NoTickHook,
    TASKS,
    { items_for(TASKS, TIMERS) },
    { lists_for(PRIOS, QUEUES, GROUPS) },
    QUEUES,
    SLOTS,
    BUFFERS,
    BYTES,
    TIMERS,
    GROUPS,
>;

/// One entry point per operation.
///
/// `$name` becomes the exported symbol, so the map attributes every byte to
/// the operation that pulled it in. `k` is a pointer the caller owns; these
/// are measured, never run.
macro_rules! op {
    ($name:ident, |$k:ident| $body:expr) => {
        /// # Safety
        /// `k` must point to a live kernel. Nothing here is ever executed.
        #[no_mangle]
        #[inline(never)]
        pub unsafe extern "C" fn $name(k: *mut K) {
            let $k: &mut K = &mut *k;
            let _ = $body;
        }
    };
}

// ------------------------------------------------------------------ tasks --
op!(kairos_task_create, |k| k.create_task("t", 1));
op!(kairos_task_delete, |k| k.task_delete(None));
op!(kairos_task_delay, |k| k.delay(10));
op!(kairos_task_delay_until, |k| {
    let mut previous = 0u64;
    k.delay_until(&mut previous, 5)
});
op!(kairos_task_suspend, |k| k.suspend(None));
op!(kairos_task_resume, |k| k.resume(TaskHandle::default()));
op!(kairos_task_priority_set, |k| k.set_priority(None, 2));
op!(kairos_task_priority_get, |k| k.task_priority_get(None));
op!(kairos_task_state_get, |k| k.task_state_get(TaskHandle::default()));
op!(kairos_task_get_handle, |k| k.task_get_handle("t"));
op!(kairos_task_abort_delay, |k| k.abort_delay(TaskHandle::default()));

// -------------------------------------------------------------- scheduler --
op!(kairos_start_scheduler, |k| k.start_scheduler());
op!(kairos_suspend_all, |k| k.suspend_all());
op!(kairos_resume_all, |k| k.resume_all());
op!(kairos_increment_tick, |k| k.increment_tick());
op!(kairos_switch_context, |k| k.switch_context());
op!(kairos_tick_count, |k| k.tick_count());
op!(kairos_check_terminated, |k| k.check_tasks_waiting_termination());

// ----------------------------------------------------- queues, semaphores --
op!(kairos_queue_create, |k| k.queue_create(4));
op!(kairos_queue_send, |k| k.queue_send(Default::default(), 1, 0));
op!(kairos_queue_receive, |k| k.queue_receive(Default::default(), 0));
op!(kairos_queue_peek, |k| k.queue_peek(Default::default(), 0));
op!(kairos_queue_messages_waiting, |k| k.queue_messages_waiting(Default::default()));
op!(kairos_queue_overwrite, |k| k.queue_overwrite(Default::default(), 2));
op!(kairos_semaphore_take, |k| k.semaphore_take(Default::default(), 0));
op!(kairos_semaphore_give, |k| k.semaphore_give(Default::default()));
op!(kairos_mutex_create, |k| k.mutex_create());
op!(kairos_mutex_create_recursive, |k| k.mutex_create_recursive());
op!(kairos_semaphore_create_counting, |k| k.semaphore_create_counting(4, 0));
op!(kairos_semaphore_create_binary, |k| k.semaphore_create_binary());
op!(kairos_mutex_take_recursive, |k| k.mutex_take_recursive(Default::default(), 0));
op!(kairos_mutex_give_recursive, |k| k.mutex_give_recursive(Default::default()));
op!(kairos_queue_send_from_isr, |k| k.queue_send_from_isr(Default::default(), 3));
op!(kairos_queue_receive_from_isr, |k| k.queue_receive_from_isr(Default::default()));

// ---------------------------------------------------------- notifications --
op!(kairos_notify, |k| k.notify(TaskHandle::default(), 0, 1, NotifyAction::SetBits));
op!(kairos_notify_wait, |k| k.notify_wait(0, 0, 0, 0));
op!(kairos_notify_take, |k| k.notify_take(0, true, 0));
op!(kairos_notify_state_clear, |k| k.notify_state_clear(None, 0));
op!(kairos_notify_from_isr, |k| k.notify_from_isr(
    TaskHandle::default(),
    0,
    1,
    NotifyAction::SetBits
));

// ----------------------------------------------------------------- timers --
op!(kairos_timer_create, |k| k.timer_create("x", 10, true, 0, 0));
op!(kairos_timer_start, |k| k.timer_start(Default::default(), 0));
op!(kairos_timer_stop, |k| k.timer_stop(Default::default(), 0));
op!(kairos_timer_reset, |k| k.timer_reset(Default::default(), 0));
op!(kairos_timer_change_period, |k| k.timer_change_period(Default::default(), 20, 0));
op!(kairos_timer_is_active, |k| k.timer_is_active(Default::default()));

// ----------------------------------------------------------- event groups --
op!(kairos_event_group_create, |k| k.event_group_create());
op!(kairos_event_group_set_bits, |k| k.event_group_set_bits(Default::default(), 1));
op!(kairos_event_group_wait_bits, |k| k.event_group_wait_bits(
    Default::default(),
    1,
    true,
    false,
    0
));
op!(kairos_event_group_clear_bits, |k| k.event_group_clear_bits(Default::default(), 1));
op!(kairos_event_group_sync, |k| k.event_group_sync(Default::default(), 1, 1, 0));

// --------------------------------------------------------- stream buffers --
op!(kairos_stream_buffer_create, |k| k.stream_buffer_create(64, 1));
op!(kairos_stream_buffer_send, |k| {
    let data = [0u8; 4];
    k.stream_buffer_send(Default::default(), &data, 0)
});
op!(kairos_stream_buffer_receive, |k| {
    let mut out = [0u8; 4];
    k.stream_buffer_receive(Default::default(), &mut out, 0)
});
op!(kairos_stream_buffer_send_from_isr, |k| {
    let data = [0u8; 4];
    k.stream_buffer_send_from_isr(Default::default(), &data)
});

/// A staticlib needs one even though nothing here is run.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
