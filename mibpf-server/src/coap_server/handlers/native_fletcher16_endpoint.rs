//! This module contains and endpoint responsible for testing out the JIT.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use coap_message::{MutableWritableMessage, ReadableMessage};
use core::convert::TryInto;
use log::debug;
use mibpf_common::{BinaryFileLayout, VMExecutionRequest};
use riot_wrappers::mutex::Mutex;

use crate::infra::suit_storage::{self, SUIT_STORAGE_SLOT_SIZE};
pub struct Fletcher16NativeHandler {
    execution_time: u32,
    result: i64,
}

impl Fletcher16NativeHandler {
    pub fn new() -> Self {
        Self {
            execution_time: 0,
            result: 0,
        }
    }

    #[inline(always)]
    fn time_now(clock: *mut riot_sys::inline::ztimer_clock_t) -> u32 {
        unsafe { riot_sys::inline::ztimer_now(clock) }
    }
}

use crate::coap_server::handlers::util::preprocess_request_raw;
use crate::vm::middleware;
use crate::vm::middleware::helpers::HelperFunction;

extern "C" {
    fn fletcher_16_80B() -> u32;
    fn fletcher_16_160B() -> u32;
    fn fletcher_16_320B() -> u32;
    fn fletcher_16_640B() -> u32;
    fn fletcher_16_1280B() -> u32;
    fn fletcher_16_2560B() -> u32;
}

impl coap_handler::Handler for Fletcher16NativeHandler {
    type RequestData = u8;

    fn extract_request_data(&mut self, request: &impl ReadableMessage) -> Self::RequestData {
        let request_data = match preprocess_request_raw(request) {
            Ok(request_data) => request_data,
            Err(code) => return code,
        };

        let Ok(request) = VMExecutionRequest::decode(request_data) else {
            return coap_numbers::code::BAD_REQUEST;
        };

        // We use a quick hack here where the size of checksummed data
        // is encoded in the length of the allowed helpers list. 1 corresponds to
        // 80B, 2 corresponds to 160B, and so on.
        let data_size = request.allowed_helpers.len();

        let test_fn = match data_size {
            1 => fletcher_16_80B,
            2 => fletcher_16_160B,
            3 => fletcher_16_320B,
            4 => fletcher_16_640B,
            5 => fletcher_16_1280B,
            6 => fletcher_16_2560B,
            _ => {
            debug!("Invalid data size: {}", data_size);
            return coap_numbers::code::BAD_REQUEST;
            }
        };

        let clock = unsafe { riot_sys::ZTIMER_USEC as *mut riot_sys::inline::ztimer_clock_t };
        let mut ret: u32;
        let start: u32 = Self::time_now(clock);
        unsafe {
            ret = test_fn();
        }
        self.execution_time = Self::time_now(clock) - start;
        debug!("JIT execution successful: {}", ret);
        self.result = ret as i64;

        coap_numbers::code::CHANGED
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        1
    }

    fn build_response(&mut self, response: &mut impl MutableWritableMessage, request: u8) {
        response.set_code(request.try_into().map_err(|_| ()).unwrap());
        let resp = format!(
            "{{\"execution_time\": {}, \"result\": {}}}",
            self.execution_time, self.result
        );
        response.set_payload(resp.as_bytes());
    }
}
