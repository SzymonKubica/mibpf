use crate::vm::{middleware, VirtualMachine};
use alloc::{
    collections::BTreeMap,
    format,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use core::{ops::DerefMut, slice::from_raw_parts_mut};
use log::debug;
use mibpf_common::{
    BinaryFileLayout, HelperAccessListSource, HelperAccessVerification, HelperFunctionID,
    VMConfiguration,
};
use mibpf_elf_utils::extract_allowed_helpers;

use rbpf::without_std::Error;

use riot_sys;
use riot_wrappers::{gcoap::PacketBuffer, mutex::Mutex, stdio::println};

use super::{
    middleware::{
        helpers::{HelperAccessList, HelperFunction},
        CoapContext,
    },
    rbpf_vm::map_interpreter,
};
use crate::infra::jit_prog_storage::{self, JIT_SLOT_SIZE};
use crate::infra::suit_storage::{self, SUIT_STORAGE_SLOT_SIZE};

/// Before we can jit-compile the program we need to adjust all .data and .rodata
/// relocations so that they point to the sections that were copied over into the
/// jit memory buffer. Because of this we need calculate the addresses of the new
/// sections and then run the relocation resolution process so that the eBPF
/// program references the data in those new section in the jitted program buffer.
/// After that is done, we can jit compile it and so all relocated memory accesses
/// will correctly point to the data/rodata located inside of the jitted program.
///
/// The reason for doing this is that we want to be able to discard the source
/// eBPF program after we jit-compile it and thus save memory as jitted programs
/// are substantially smaller.
static PROGRAM_COPY_BUFFER: Mutex<[u8; JIT_SLOT_SIZE]> = Mutex::new([0; JIT_SLOT_SIZE]);

pub struct RbpfJIT<'a> {
    pub program: Option<&'a [u8]>,
    pub layout: BinaryFileLayout,
    pub allowed_helpers: Vec<HelperFunctionID>,
    pub helper_access_verification: HelperAccessVerification,
    pub helper_access_list_source: HelperAccessListSource,
    pub recompile: bool,
    pub jit_prog_slot: usize,
    pub jit_program_length: usize,
}

impl<'a> RbpfJIT<'a> {
    pub fn new(config: VMConfiguration, allowed_helpers: Vec<HelperFunctionID>) -> RbpfJIT<'a> {
        RbpfJIT {
            program: None,
            layout: config.binary_layout,
            allowed_helpers,
            helper_access_verification: config.helper_access_verification,
            helper_access_list_source: config.helper_access_list_source,
            recompile: config.jit_compile,
            jit_prog_slot: config.suit_slot,
            jit_program_length: 0,
        }
    }
}

impl<'a> VirtualMachine<'a> for RbpfJIT<'a> {
    fn initialize_vm(&mut self, program: &'a mut [u8]) -> Result<(), String> {
        self.program = Some(program);
        if !self.recompile {
            return Ok(());
        }
        if self.layout != BinaryFileLayout::RawObjectFile {
            Err("The JIT only supports raw object file binary layout")?;
        };
        // We take the list of helpers from the execute request as this is the
        // only one way supported by the raw elf file binary layout that we use for the JIT.
        let mut helpers_map = BTreeMap::new();
        let helper_access_list = HelperAccessList::from(self.allowed_helpers.clone());

        for h in helper_access_list.0 {
            helpers_map.insert(h.id as u32, h.function);
        }

        let jit_slot = self.jit_prog_slot;

        // Here we acquire a pointer to global storage where the jitted
        // program will be written. The additional scope is introduced so
        // that the acquired MutexGuard goes out of scope at the end of it
        // and so the lock is released. (RAII)
        let mut slot_guard = jit_prog_storage::acquire_storage_slot(jit_slot).unwrap();
        let mut text_offset = 0;
        {
            let mut jit_memory = rbpf::JitMemory::new(
                program,
                PROGRAM_COPY_BUFFER.lock().as_mut(),
                slot_guard.0.as_mut(),
                &helpers_map,
                false,
                false,
                rbpf::InterpreterVariant::RawObjectFile,
            )
            .unwrap();
            self.jit_program_length = jit_memory.offset;
            debug!("JIT compilation successful");
            debug!("jitted program size: {} [B]", jit_memory.offset);
            text_offset = jit_memory.text_offset;
        }
        slot_guard.1 = text_offset;
        Ok(())
    }
    fn verify(&self) -> Result<(), String> {
        let mut vm =
            rbpf::EbpfVmMbuff::new(Some(self.program.unwrap()), map_interpreter(self.layout))
                .map_err(|e| format!("Error: {:?}", e))?;
        middleware::helpers::register_helpers(
            &mut vm,
            HelperAccessList::from(self.allowed_helpers.clone()).0,
        );

        vm.verify_loaded_program()
            .map_err(|e| format!("Error: {:?}", e))?;

        if self.helper_access_verification == HelperAccessVerification::PreFlight {
            let interpreter = map_interpreter(self.layout);
            let helpers_idxs = self
                .allowed_helpers
                .iter()
                .map(|id| *id as u32)
                .collect::<Vec<u32>>();
            vm.verify_helper_calls(&helpers_idxs, interpreter)
                .map_err(|e| format!("Error when checking helper function access: {:?}", e))?;
        }
        Ok(())
    }

    fn execute(&mut self) -> Result<u64, String> {
        let jitted_fn = jit_prog_storage::get_program_from_slot(self.jit_prog_slot).unwrap();

        let mut ret = 0;
        unsafe {
            // We don't pass any meaningful arguments here as the program doesn't
            // work on a COAP message packet buffer.
            ret = jitted_fn(0 as *mut u8, 0, 0 as *mut u8, 0);
        }
        jit_prog_storage::free_storage_slot(self.jit_prog_slot);
        debug!("JIT execution successful: {}", ret);
        Ok(ret as u64)
    }

    fn execute_on_coap_pkt(&mut self, pkt: &mut PacketBuffer) -> Result<u64, String> {
        todo!()
    }

    fn get_program_length(&self) -> usize {
        self.jit_program_length
    }
}