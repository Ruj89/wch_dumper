#![allow(unused)]

//! USB MTP constants.
pub const USB_CLASS_APPN_SPEC: u8 = 0x06;
pub const APPN_SPEC_SUBCLASS_MTP: u8 = 0x01;
pub const MTP_PROTOCOL_MTP: u8 = 0x01;

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
#[allow(unused)]
pub enum State {
    AppIdle = 0,
    AppDetach = 1,
    MtpIdle = 2,
    DownloadSync = 3,
    DownloadBusy = 4,
    DownloadIdle = 5,
    ManifestSync = 6,
    Manifest = 7,
    ManifestWaitReset = 8,
    UploadIdle = 9,
    Error = 10,
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
#[allow(unused)]
pub enum MtpStatus {
    Ok = 0x00,
    ErrTarget = 0x01,
    ErrFile = 0x02,
    ErrWrite = 0x03,
    ErrErase = 0x04,
    ErrCheckErased = 0x05,
    ErrProg = 0x06,
    ErrVerify = 0x07,
    ErrAddress = 0x08,
    ErrNotDone = 0x09,
    ErrFirmware = 0x0A,
    ErrVendor = 0x0B,
    ErrUsbr = 0x0C,
    ErrPor = 0x0D,
    ErrUnknown = 0x0E,
    ErrStalledPkt = 0x0F,
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum MtpRequest {
    Detach = 0,
    Dnload = 1,
    Upload = 2,
    GetStatus = 3,
    ClrStatus = 4,
    GetState = 5,
    Abort = 6,
}

impl TryFrom<u8> for MtpRequest {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MtpRequest::Detach),
            1 => Ok(MtpRequest::Dnload),
            2 => Ok(MtpRequest::Upload),
            3 => Ok(MtpRequest::GetStatus),
            4 => Ok(MtpRequest::ClrStatus),
            5 => Ok(MtpRequest::GetState),
            6 => Ok(MtpRequest::Abort),
            _ => Err(()),
        }
    }
}