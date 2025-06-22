#![no_std]

use super::consts as consts; 
use consts::{MtpRequest};

/// USB Device in MTP mode
pub struct UsbMtpDevice {
}

impl<'a> UsbMtpDevice {
    pub fn new() -> Self {
        UsbMtpDevice {
        }
    }

    pub fn handle_mtp_in<'b>(
        &mut self,
        _req: MtpRequest,
        _buf: &'b mut [u8],
    ) -> Result<&'b [u8], ()> {
        Err(())
    }
}