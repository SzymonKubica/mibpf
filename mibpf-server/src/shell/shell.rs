use alloc::sync::Arc;
use riot_wrappers::msg::v2::SendPort;
use riot_wrappers::mutex::Mutex;
use riot_wrappers::shell::CommandList;

use riot_wrappers::{cstr::cstr, stdio::println};

use crate::shell::{bpf_command, gpio_command};
use crate::vm::{VMExecutionRequest, VM_EXECUTION_REQUEST_TYPE};

pub fn shell_main(
    execution_send: &Arc<Mutex<SendPort<VMExecutionRequest, VM_EXECUTION_REQUEST_TYPE>>>,
) -> Result<(), ()> {
    let mut line_buf = [0u8; 128];

    // TODO: add the command to execute loaded bpf programs
    let mut commands = riot_shell_commands::all();

    let bpf_handler = bpf_command::VMExecutionShellCommandHandler::new(execution_send.clone());

    let commands = trait_identity(commands).and(
        cstr!("gpio"),
        cstr!("Access GPIO pins"),
        gpio_command::handle_command,
    );

    let commands = trait_identity(commands).and(
        cstr!("bpf-execute"),
        cstr!("Execute and manage eBPF programs"),
        |stdio: &mut _, args: riot_wrappers::shell::Args<'_>| {
            bpf_handler.handle_command(stdio, args);
        },
    );

    trait_identity(commands).run_forever(&mut line_buf);
    unreachable!();
}

// Workaround for a bug described here: https://github.com/RIOT-OS/rust-riot-wrappers/issues/76
fn trait_identity(mut c: impl CommandList) -> impl CommandList {
    c
}
