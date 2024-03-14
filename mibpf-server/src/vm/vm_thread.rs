use core::ffi::c_void;

use alloc::{boxed::Box, format, sync::Arc, vec::Vec};
use log::{debug, error, info};

use riot_wrappers::{
    cstr::cstr,
    msg::v2::{MessageSemantics, NoConfiguredMessages, Processing, ReceivePort, SendPort},
    mutex::{Mutex, MutexGuard},
    stdio::println,
    thread::{self, CountedThread, CountingThreadScope},
};

use riot_sys;
use riot_sys::msg_t;

use crate::{
    infra::suit_storage,
    model::{
        enumerations::TargetVM,
        requests::{VMExecutionCompleteMsg, VMExecutionRequest, VMExecutionRequestMsg},
    },
    spawn_thread,
    vm::{middleware, FemtoContainerVm, RbpfVm, VirtualMachine},
};

static VM_WORKER_0_STACK: Mutex<[u8; 4096]> = Mutex::new([0; 4096]);
static VM_WORKER_1_STACK: Mutex<[u8; 4096]> = Mutex::new([0; 4096]);
static VM_WORKER_2_STACK: Mutex<[u8; 4096]> = Mutex::new([0; 4096]);
static VM_WORKER_3_STACK: Mutex<[u8; 4096]> = Mutex::new([0; 4096]);

/// The unique identifier of the request type used to start the execution of the VM.
pub const VM_EXEC_REQUEST: u16 = 23;
pub const VM_COMPLETE_NOTIFICATION: u16 = 24;
pub type VMExecutionRequestPort = ReceivePort<VMExecutionRequestMsg, VM_EXEC_REQUEST>;
pub type VMExecutionCompletePort = ReceivePort<VMExecutionCompleteMsg, VM_COMPLETE_NOTIFICATION>;
pub type ExecutionSendPortHandle = Arc<Mutex<SendPort<VMExecutionRequestMsg, VM_EXEC_REQUEST>>>;
pub type CompletionSendPortHandle =
    Arc<Mutex<SendPort<VMExecutionCompleteMsg, VM_COMPLETE_NOTIFICATION>>>;

/// Responsible for managing execution of long-running eBPF programs. It receives
/// messages from other parts of the system that are requesting that a particular
/// instance of the VM should be started and execute a specified program.
pub struct VMExecutionManager {
    request_receive_port: VMExecutionRequestPort,
    request_send_port: ExecutionSendPortHandle,
    notification_receive_port: VMExecutionCompletePort,
    notification_send_port: CompletionSendPortHandle,
    message_semantics: Processing<
        Processing<NoConfiguredMessages, VMExecutionRequestMsg, VM_EXEC_REQUEST>,
        VMExecutionCompleteMsg,
        VM_COMPLETE_NOTIFICATION,
    >,
}

impl VMExecutionManager {
    pub fn new(message_semantics: NoConfiguredMessages) -> Self {
        let (message_semantics, receive_port, send_port): (_, VMExecutionRequestPort, _) =
            message_semantics.split_off();

        let (message_semantics, receive_port_2, send_port_2): (_, VMExecutionCompletePort, _) =
            message_semantics.split_off();

        VMExecutionManager {
            request_receive_port: receive_port,
            request_send_port: Arc::new(Mutex::new(send_port)),
            notification_receive_port: receive_port_2,
            notification_send_port: Arc::new(Mutex::new(send_port_2)),
            message_semantics,
        }
    }

    /// Returns an atomically-counted reference to the send end of the message
    /// channel for sending requests to execute eBPF programs.
    pub fn get_send_port(&self) -> Arc<Mutex<SendPort<VMExecutionRequestMsg, VM_EXEC_REQUEST>>> {
        self.request_send_port.clone()
    }

    /// This is the main function of the thread that allow for executing long-running
    /// eBPF programs. It spawns worker threads and then sends messages to them to
    /// start executing long running eBPF programs.
    pub fn start(&self) {
        extern "C" {
            fn bpf_store_init();
        }

        // We need to initialise the global storage for the VM helpers.
        // Currently we repurpose the Femto-Container implementation
        unsafe {
            bpf_store_init();
        }

        let mut worker_0_stack = VM_WORKER_0_STACK.lock();
        let mut worker_1_stack = VM_WORKER_1_STACK.lock();
        let mut worker_2_stack = VM_WORKER_2_STACK.lock();
        let mut worker_3_stack = VM_WORKER_3_STACK.lock();

        let notification_port = self.notification_send_port.clone();

        let mut worker_0_main = || vm_main_thread(&notification_port);
        let mut worker_1_main = || vm_main_thread(&notification_port);
        let mut worker_2_main = || vm_main_thread(&notification_port);
        let mut worker_3_main = || vm_main_thread(&notification_port);

        thread::scope(|ts| {
            let pri = riot_sys::THREAD_PRIORITY_MAIN;
            // All worker threads need to be spawned at the start because the
            // thread scope doesn't allow for spawning new threads on the fly,
            // we always need to know the number of threads at the start.
            // We need to set different priorities for different workers because
            // otherwise they will keep blocking each other.
            let worker_0 = spawn_thread!(ts, "Worker 0", worker_0_stack, worker_0_main, pri - 4);
            let worker_1 = spawn_thread!(ts, "Worker 1", worker_1_stack, worker_1_main, pri - 3);
            let worker_2 = spawn_thread!(ts, "Worker 2", worker_2_stack, worker_2_main, pri - 2);
            let worker_3 = spawn_thread!(ts, "Worker 3", worker_3_stack, worker_3_main, pri - 1);

            let mut free_workers: Vec<i16> = alloc::vec![
                worker_0.pid().into(),
                worker_1.pid().into(),
                worker_2.pid().into(),
                worker_3.pid().into(),
            ];

            loop {
                let code = self
                    .message_semantics
                    .receive()
                    .decode(
                        &self.request_receive_port,
                        |_s, mut execution_request| unsafe {
                            // for now we route slot 0 to worker 0 and slot 1 to worker 1
                            let target = TargetVM::from(execution_request.vm_target);
                            if free_workers.is_empty() {
                                error!("No free workers to execute the request.");
                                return;
                            }
                            let pid: riot_sys::kernel_pid_t = free_workers.pop().unwrap();
                            info!("Sending execution request to the worker with PID: {}", pid);
                            let mut msg: msg_t = execution_request.into();
                            riot_sys::msg_send(&mut msg, pid);
                        },
                    )
                    .unwrap_or_else(|_m| {
                        _m.decode(&self.notification_receive_port, |_s, mut notification| {
                            info!(
                                "Received notification from worker with PID: {}",
                                notification.worker_pid
                            );
                            info!("Adding worker back to the pool of free workers.");
                            free_workers.push(notification.worker_pid)
                        })
                        .unwrap_or_else(|_m| {
                            error!("Failed to decode message.");
                        });
                    });
                println!("Result code {:?}", code);
            }
        });
    }
}

fn vm_main_thread(send_port: &CompletionSendPortHandle) {
    loop {
        let mut msg: msg_t = Default::default();
        unsafe {
            let _ = riot_sys::msg_receive(&mut msg);
        }
        let execution_request_msg: &VMExecutionRequestMsg = msg.into();
        let execution_request = VMExecutionRequest::from(execution_request_msg);

        let mut program_buffer: [u8; 1024] = [0; 1024];
        let program = suit_storage::load_program(&mut program_buffer, execution_request.suit_slot);

        info!(
            "Loaded program bytecode from SUIT storage slot {}, program length: {}",
            execution_request.suit_slot,
            program.len()
        );

        let vm: Box<dyn VirtualMachine> = match execution_request.vm_target {
            TargetVM::Rbpf => Box::new(RbpfVm::new(
                Vec::from(middleware::ALL_HELPERS),
                execution_request.binary_layout,
            )),
            TargetVM::FemtoContainer => Box::new(FemtoContainerVm {}),
        };

        let mut result: i64 = 0;
        let execution_time = vm.execute(&program, &mut result);

        let resp = format!("Execution_time: {}, result: {}", execution_time, result);
        println!("{}", &resp);

        if let Ok(()) = send_port.lock().try_send(VMExecutionCompleteMsg {
            worker_pid: riot_wrappers::thread::get_pid().into(),
        }) {
            info!("VM execution completion notification sent successfully");
        } else {
            error!("Failed to send notification message.");
        }
    }
}
