use alloc::{boxed::Box, format, string::String, vec::Vec};
use mibpf_common::{
    BinaryFileLayout, HelperAccessVerification, HelperFunctionID, TargetVM, VMConfiguration,
};
use mibpf_elf_utils::{extract_allowed_helpers, resolve_relocations};
use riot_wrappers::gcoap::PacketBuffer;

use crate::infra::{local_storage, suit_storage};

use super::{middleware::helpers::HelperAccessList, rbpf_vm, FemtoContainerVm, RbpfVm};

/// Structs implementing this interface should allow for executing eBPF programs
/// both raw and with access to the incoming CoAP packet.
pub trait VirtualMachine<'a> {
    /// Loads, verifies, optionally resolves relocations and executes the program.
    fn full_run(&mut self, program: &'a mut [u8]) -> Result<u64, String> {
        let patched_program = self.resolve_relocations(program)?;
        self.initialise_vm(patched_program)?;
        self.verify()?;
        self.execute()
    }
    fn full_run_on_coap_pkt(
        &mut self,
        program: &'a mut [u8],
        pkt: &mut PacketBuffer,
    ) -> Result<u64, String> {
        let patched_program = self.resolve_relocations(program)?;
        self.initialise_vm(patched_program)?;
        self.verify()?;
        self.execute_on_coap_pkt(pkt)
    }
    /// Patches the program bytecode using the relocation metadata to fix accesses
    /// to .data and .rodata sections.
    fn resolve_relocations(&mut self, program: &'a mut [u8]) -> Result<&'a [u8], String>;
    /// Verifies the program bytecode after it has been loaded into the VM.
    fn verify(&self) -> Result<(), String>;
    fn initialise_vm(&mut self, program: &'a [u8]) -> Result<(), String>;
    /// Executes a given program and returns its return value.
    fn execute(&mut self) -> Result<u64, String>;
    /// Executes a given eBPF program giving it access to the provided PacketBuffer
    /// and returns the return value of the program. The value returned
    /// by the program needs to represent the length of
    /// the packet PDU + payload. The reason for this is that the handler then
    /// needs to know this length when sending the response back.
    fn execute_on_coap_pkt(&mut self, pkt: &mut PacketBuffer) -> Result<u64, String>;
}

/// Responsible for initialising the VM. It loads the program bytecode from the
/// SUIT storage, and initialises the correct version of the VM struct.
/// The reason we do both of those things at the same time is that the lifetime
/// of the VM is tied to the lifetime of the program buffer (as every VM operates
/// on only one program).
pub fn initialize_vm<'a>(
    config: VMConfiguration,
    allowed_helpers: Vec<HelperFunctionID>,
    program_buffer: &'a mut [u8],
) -> Result<(&mut [u8], Box<dyn VirtualMachine<'a> + 'a>), String> {
    let mut program = suit_storage::load_program(program_buffer, config.suit_slot);

    match config.vm_target {
        TargetVM::Rbpf => {
            return Ok((program, Box::new(RbpfVm::new(config, allowed_helpers)?)));
        }
        TargetVM::FemtoContainer => {
            return Ok((program, Box::new(FemtoContainerVm::new())));
        }
    }
}
