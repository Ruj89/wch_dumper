//! MTP class implementation.

use core::iter;

use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder};
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use serde::{Serialize, Deserialize};

use crate::dumper::{Msg, MsgStartConsole};

/// This should be used as `device_class` when building the `UsbDevice`.
const USB_CLASS_MTP: u8 = 0x06;
const MTP_SUBCLASS: u8 = 0x01;
const MTP_PROTOCOL: u8 = 0x01;

#[derive(Debug)]
pub struct PtpCommand<'a> {
    pub op_code: u16,
    pub transaction_id: u32,
    pub payload: &'a [u8],
}

/// Errors returned by [`MtpClass::parse_mtp_command`]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MtpError {
    CannotParseHeader,
    WrongPacketType
}

#[repr(u16)]
enum MtpCommandError {
    Ok = 0x2001,
    // SessionNotOpen = 0x2003,
    // InvalidTransactionId = 0x2004,
    OperationNotSupported = 0x2005,
    // ParameterNotSupported = 0x2006,
    // InvalidStorageId = 0x2008,
    InvalidObjectFormatCode = 0x200B,
    // StoreFull = 0x200C,
    // StoreReadOnly = 0x200E,
    // AccessDenied = 0x200F,
    StoreNotAvailable = 0x2013,
    InvalidParentObject = 0x201A,
    ObjectTooLarge = 0xA809,
}

#[repr(u16)]
pub enum MtpContainerType {
    // Undefined = 0x0000,
    Command = 0x0001,
    Data = 0x0002,
    Response = 0x0003,
    // Event = 0x0004,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DumperConfig {
    pub mapper: u8,
    pub prgsize: u8,
    pub chrsize: u8,
    pub prg: u16, // KB
    pub chr: u16, // KB
}

/// Packet level implementation of a MTP serial port.
///
/// This class can be used directly and it has the least overhead due to directly reading and
/// writing USB packets with no intermediate buffers, but it will not act like a stream-like serial
/// port. The following constraints must be followed if you use this class directly:
///
/// - `read_packet` must be called with a buffer large enough to hold `max_packet_size` bytes.
/// - `write_packet` must not be called with a buffer larger than `max_packet_size` bytes.
/// - If you write a packet that is exactly `max_packet_size` bytes long, it won't be processed by the
///   host operating system until a subsequent shorter packet is sent. A zero-length packet (ZLP)
///   can be sent if there is no other data to send. This is because USB bulk transactions must be
///   terminated with a short packet, even if the bulk endpoint is used for stream-like data.
pub struct MtpClass<'d, D: Driver<'d>> {
    //_comm_ep: D::EndpointIn,
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
    in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    configuration_file: &'d mut [u8],
    configuration_file_size: usize,
    configuration_file_deleted: bool,
}

impl<'d, D: Driver<'d>> MtpClass<'d, D> {
    /// Creates a new MtpClass with the provided UsbBus and `max_packet_size` in bytes. For
    /// full-speed devices, `max_packet_size` has to be one of 8, 16, 32 or 64.
    pub fn new(builder: &mut Builder<'d, D>,
        max_packet_size: u16,
        in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        configuration_file: &'d mut [u8]) -> Self {
        assert!(builder.control_buf_len() >= 7);

        let mut func = builder.function(0x00, 0x00, 0x00);
        let mut iface = func.interface();
        let mut alt = iface.alt_setting(USB_CLASS_MTP, MTP_SUBCLASS, MTP_PROTOCOL, None);
        let read_ep = alt.endpoint_bulk_out(max_packet_size);
        let write_ep = alt.endpoint_bulk_in(max_packet_size);
        //let comm_ep = alt.endpoint_interrupt_in(8, 255);

        drop(func);

        let config = DumperConfig {
            mapper: 1,
            prgsize: 3,
            chrsize: 0,
            prg: 128,
            chr: 0
        };

        let configuration_file_size = serde_json_core::to_slice(&config, configuration_file).unwrap();
        MtpClass {
            //_comm_ep: comm_ep,
            read_ep,
            write_ep,
            in_channel,
            out_channel,
            configuration_file,
            configuration_file_size,
            configuration_file_deleted: false,
        }
    }

    /// Gets the maximum packet size in bytes.
    pub fn max_packet_size(&self) -> usize {
        // The size is the same for both endpoints.
        self.read_ep.info().max_packet_size.into()
    }

    /// Writes a single packet into the IN endpoint.
    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        let len = core::cmp::min(data.len(), self.max_packet_size() as usize);
        self.write_ep.write(&data[..len]).await
    }

    /// Reads a single packet from the OUT endpoint.
    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    /// Waits for the USB host to enable this interface
    pub async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }

    pub fn parse_mtp_command<'a>(&self, buf: &'a[u8], phase: MtpContainerType) -> Result<PtpCommand<'a>, MtpError> {
        let length = usize::from_le_bytes(buf[0..4].try_into().unwrap());
        if length < 12 {
            return Err(MtpError::CannotParseHeader);
        }
        let packet_type = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        let op_code = u16::from_le_bytes(buf[6..8].try_into().unwrap());
        let transaction_id = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        let payload = &buf[12..length];

        if packet_type != phase as u16 {
            return Err(MtpError::WrongPacketType);
        }

        Ok(PtpCommand {
            op_code,
            transaction_id,
            payload,
        })
    }

    // Helper: write little-endian u8
    fn write_u8(buf: &mut [u8], offset: &mut usize, val: u8) {
        buf[*offset] = val;
        *offset += 1;
    }

    // Helper: write little-endian u16
    fn write_u16(buf: &mut [u8], offset: &mut usize, val: u16) {
        buf[*offset..*offset + 2].copy_from_slice(&val.to_le_bytes());
        *offset += 2;
    }

    // Helper: write little-endian u32
    fn write_u32(buf: &mut [u8], offset: &mut usize, val: u32) {
        buf[*offset..*offset + 4].copy_from_slice(&val.to_le_bytes());
        *offset += 4;
    }

    // Helper: write little-endian u64
    fn write_u64(buf: &mut [u8], offset: &mut usize, val: u64) {
        buf[*offset..*offset + 8].copy_from_slice(&val.to_le_bytes());
        *offset += 8;
    }

    // Helper: write buffer
    fn write_buffer(buf: &mut [u8], offset: &mut usize, in_buf: &[u8]) {
        buf[*offset..*offset + in_buf.len()].copy_from_slice(&in_buf);
        *offset += in_buf.len();
    }

    // PTP string format: len (u8), UTF-16LE chars, 0x0000 terminator
    fn write_string(buf: &mut [u8], offset: &mut usize, s: &str) {
        if s.len() == 0 {
            buf[*offset] = 0;
            *offset += 1;
            return
        }

        buf[*offset] = (s.len() + 1) as u8; // total chars incl. null
        *offset += 1;

        for c in s.encode_utf16() {
            let b = c.to_le_bytes();
            buf[*offset] = b[0];
            buf[*offset + 1] = b[1];
            *offset += 2;
        }

        // null terminator UTF-16
        buf[*offset] = 0;
        buf[*offset + 1] = 0;
        *offset += 2;
    }

    fn generate_ok_response_block(&self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let mut offset = 0;
        Self::write_u32(buffer, &mut offset, 12u32);
        Self::write_u16(buffer, &mut offset, MtpContainerType::Response as u16);
        Self::write_u16(buffer, &mut offset, MtpCommandError::Ok as u16);
        Self::write_u32(buffer, &mut offset, transaction_id);
        offset
    }

    fn generate_error_response_block(&self, transaction_id: u32, buffer: &mut [u8], error: MtpCommandError) -> usize {
        let mut offset = 0;
        Self::write_u32(buffer, &mut offset, 12u32);
        Self::write_u16(buffer, &mut offset, MtpContainerType::Response as u16);
        Self::write_u16(buffer, &mut offset, error as u16);
        Self::write_u32(buffer, &mut offset, transaction_id);
        offset
    }

    fn generate_device_info_response(&self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let mut offset = 12;
        Self::write_u16(buffer, &mut offset, 110); // StandardVersion
        Self::write_u32(buffer, &mut offset, 6); // VendorExtensionID = 6 (Microsoft)
        Self::write_u16(buffer, &mut offset, 100);  // VendorExtensionVersion
        Self::write_string(buffer, &mut offset, "microsoft.com: 1.0"); // VendorExtensionDesc
        Self::write_u16(buffer, &mut offset, 0); // FunctionalMode
        let supported_operations = [
            0x1001, 0x1002, 0x1003, 0x1004, 0x1005, 0x1006, 0x1007, 0x1008, 0x1009, 0x100A,
            0x100B, 0x100C, 0x100D, 0x100E, 0x100F, 0x1010, 0x1011, 0x1012, 0x1013, 0x1014,
            0x1015, 0x1016, 0x1017, 0x1018, 0x1019, 0x101A, 0x101B, 0x101C, 0x9801, 0x9802,
            0x9803, 0x9804, 0x9810, 0x9811, 0x9820, 0x9805, 0x9806, 0x9807, 0x9808,
        ];
        Self::write_u32(buffer, &mut offset, supported_operations.len().try_into().unwrap()); // NumOperationsSupported
        for operation in supported_operations  {
            Self::write_u16(buffer, &mut offset, operation); // OperationSupported
        }
        let supported_events = [
            0x4000, 0x4001, 0x4002, 0x4003, 0x4004, 0x4005, 0x4006, 0x4007, 0x4008, 0x4009,
            0x400A, 0x400B, 0x400C, 0x400D, 0x400E, 0xC801, 0xC802, 0xC803,
        ];
        Self::write_u32(buffer, &mut offset, supported_events.len().try_into().unwrap()); // NumEventsSupported
        for event in supported_events  {
            Self::write_u16(buffer, &mut offset, event); // EventSupported
        }
        let supported_device_properties = [
            0xd401, 0xd402, 0x5002, 0x5011,
        ];
        Self::write_u32(buffer, &mut offset, supported_device_properties.len().try_into().unwrap()); // NumDevicePropertiesSupported
        for device_property in supported_device_properties  {
            Self::write_u16(buffer, &mut offset, device_property); // DevicePropertiesSupported
        }
        Self::write_u32(buffer, &mut offset, 0); // CaptureFormats = empty
        let supported_playbacks = [
            0x3000, 0x3001, 0x3004, 0x3005, 0x3008, 0x3009, 0x300b, 0x3801, 0x3802, 0x3804,
            0x3807, 0x3808, 0x380b, 0x380d, 0xb901, 0xb902, 0xb903, 0xb982, 0xb983, 0xb984,
            0xba05, 0xba10, 0xba11, 0xba14, 0xba82, 0xb906, 0x3811, 0x3812,
        ];
        Self::write_u32(buffer, &mut offset, supported_playbacks.len().try_into().unwrap()); // NumPlaybackSupported
        for playback in supported_playbacks  {
            Self::write_u16(buffer, &mut offset, playback); // PlaybackSupported
        }
        Self::write_string(buffer, &mut offset, "arkHive"); // Manufacturer
        Self::write_string(buffer, &mut offset, "MTP Dumper"); // Model
        Self::write_string(buffer, &mut offset, "1.0"); // DeviceVersion
        Self::write_string(buffer, &mut offset, "12345678"); // SerialNumber
        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1001);    // Operation: GetDeviceInfo
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    fn generate_storage_id_response(&self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let mut offset = 12;
        Self::write_u32(buffer, &mut offset, 1); // NumStorageIDs
        Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1004);    // Operation: GetStorageIDs
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    fn generate_storage_info_response<'a>(&self, transaction_id: u32, buffer: &mut [u8], cmd: &PtpCommand<'a>) -> usize {
        let storage_id= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        if storage_id != 0x00010001 {
            return 0;
        }

        let mut offset = 12;
        Self::write_u16(buffer, &mut offset, 0x0004); // Storage Type = Removable RAM
        Self::write_u16(buffer, &mut offset, 0x0002); // Filesystem Type = Generic hierarchical
        Self::write_u16(buffer, &mut offset, 0x0000); // Access Capability = Read-only without object deletion
        Self::write_u64(buffer, &mut offset, u64::max_value()); // Max Capacity > TB
        Self::write_u64(buffer, &mut offset, 0); // Free Space In Bytes
        Self::write_u32(buffer, &mut offset, 0xFFFFFFFF); // *Free Space In Objects = Not used
        Self::write_string(buffer, &mut offset, "ROMs"); // Storage Description
        Self::write_string(buffer, &mut offset, ""); // Volume Identifier

        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1005);    // Operation: GetStorageIDs
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    fn object_format_codes_contains(cmd: &PtpCommand, needle: u16) -> bool {
        let object_format_code_count= u32::from_le_bytes(cmd.payload[4..8].try_into().unwrap());
        if object_format_code_count == 0 {
            return true;
        }
        let object_format_code_offset = 8;
        for object_format_code_index in 0..object_format_code_count {
            let buffer_index = object_format_code_offset + (object_format_code_index * 2) as usize;
            let object_format_code = u16::from_le_bytes([cmd.payload[buffer_index], cmd.payload[buffer_index+1]]);
            if object_format_code == needle {
                return true;
            }
        }
        return false;
    }

    fn object_handle_of_association_contains(cmd: &PtpCommand, needle: u32) -> bool {
        let object_format_code_count= u32::from_le_bytes(cmd.payload[4..8].try_into().unwrap());
        let object_format_code_offset = 8;
        let object_handle_of_association_offset = object_format_code_offset + (object_format_code_count * 2) as usize;
        let object_handle_of_association= u32::from_le_bytes(cmd.payload[
            object_handle_of_association_offset..object_handle_of_association_offset+4
            ].try_into().unwrap());
        if object_handle_of_association == 0 {
            return true;
        }
        return needle == object_handle_of_association;
    }

    fn generate_object_handles_response<'a>(&self, transaction_id: u32, buffer: &mut [u8], cmd: &PtpCommand<'a>) -> usize {
        let mut offset = 12;
        let storage_id= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        let mut object_handle_offset = offset;
        offset += 4;
        let mut object_handle_count = 0;
        if (storage_id == 0xFFFFFFFF || storage_id == 0x00010001) &&
            Self::object_format_codes_contains(cmd, 0x3001) &&
            Self::object_handle_of_association_contains(cmd, 0xFFFFFFFF) {
                Self::write_u32(buffer, &mut offset, 0x00000001); // ObjectHandle[0] id
                Self::write_u32(buffer, &mut offset, 0x00000004); // ObjectHandle[0] id
                object_handle_count += 2;
        }
        if (storage_id == 0xFFFFFFFF || storage_id == 0x00010001) &&
            Self::object_format_codes_contains(cmd, 0x3000) {
            if Self::object_handle_of_association_contains(cmd, 0x00000001) {
                Self::write_u32(buffer, &mut offset, 0x00000002); // ObjectHandle[0] id
                object_handle_count += 1;
                if !self.configuration_file_deleted {
                    Self::write_u32(buffer, &mut offset, 0x00000003); // ObjectHandle[0] id
                    object_handle_count += 1;
                }
            }
            if Self::object_handle_of_association_contains(cmd, 0x00000004) {
                Self::write_u32(buffer, &mut offset, 0x00000005); // ObjectHandle[0] id
                object_handle_count += 1;
            }
        }
        Self::write_u32(buffer, &mut object_handle_offset, object_handle_count); // NumObjectHandles
        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1007);    // Operation: GetStorageIDs
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    fn generate_object_info_response<'a>(&self, transaction_id: u32, buffer: &mut [u8], cmd: &PtpCommand<'a>) -> usize {
        let object_handle= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        let mut offset = 12;
        match object_handle  {
            0x00000001 => {
                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
                Self::write_u16(buffer, &mut offset, 0x3001); // Object Format
                Self::write_u16(buffer, &mut offset, 0x0001); // Protection Status
                Self::write_u32(buffer, &mut offset, 0); // Object Compressed Size
                Self::write_u16(buffer, &mut offset, 0x3001); // Thumb Format
                Self::write_u32(buffer, &mut offset, 0); // Thumb Compressed Size
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Bit Depth
                Self::write_u32(buffer, &mut offset, 0x00000000); // Parent Object
                Self::write_u16(buffer, &mut offset, 0x0001); // Association Type
                Self::write_u32(buffer, &mut offset, 0); // Association Description
                Self::write_u32(buffer, &mut offset, 0); // Sequence Number
                Self::write_string(buffer, &mut offset, "NES"); // Filename
                Self::write_string(buffer, &mut offset, "20250714T173222.0Z"); // Date Created
                Self::write_string(buffer, &mut offset, "20250715T183222.0Z"); // Date Modified
                Self::write_string(buffer, &mut offset, "0"); // Keywords
            }
            0x00000002 => {
                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
                Self::write_u16(buffer, &mut offset, 0x3000); // Object Format
                Self::write_u16(buffer, &mut offset, 0x0001); // Protection Status
                Self::write_u32(buffer, &mut offset, 0x8000+0x2000+16); // Object Compressed Size
                Self::write_u16(buffer, &mut offset, 0x3000); // Thumb Format
                Self::write_u32(buffer, &mut offset, 0); // Thumb Compressed Size
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Bit Depth
                Self::write_u32(buffer, &mut offset, 0x00000001); // Parent Object
                Self::write_u16(buffer, &mut offset, 0); // Association Type
                Self::write_u32(buffer, &mut offset, 0); // Association Description
                Self::write_u32(buffer, &mut offset, 0); // Sequence Number
                Self::write_string(buffer, &mut offset, "rom.nes"); // Filename
                Self::write_string(buffer, &mut offset, "20250714T173222.0Z"); // Date Created
                Self::write_string(buffer, &mut offset, "20250715T183222.0Z"); // Date Modified
                Self::write_string(buffer, &mut offset, "0"); // Keywords
            }
            0x00000003 => {
                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
                Self::write_u16(buffer, &mut offset, 0x3000); // Object Format
                Self::write_u16(buffer, &mut offset, 0x0000); // Protection Status
                Self::write_u32(buffer, &mut offset, self.configuration_file_size as u32); // Object Compressed Size
                Self::write_u16(buffer, &mut offset, 0x3000); // Thumb Format
                Self::write_u32(buffer, &mut offset, 0); // Thumb Compressed Size
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Bit Depth
                Self::write_u32(buffer, &mut offset, 0x00000001); // Parent Object
                Self::write_u16(buffer, &mut offset, 0); // Association Type
                Self::write_u32(buffer, &mut offset, 0); // Association Description
                Self::write_u32(buffer, &mut offset, 0); // Sequence Number
                Self::write_string(buffer, &mut offset, "config.json"); // Filename
                Self::write_string(buffer, &mut offset, "20250714T173222.0Z"); // Date Created
                Self::write_string(buffer, &mut offset, "20250715T183222.0Z"); // Date Modified
                Self::write_string(buffer, &mut offset, "0"); // Keywords
            }

            0x00000004 => {
                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
                Self::write_u16(buffer, &mut offset, 0x3001); // Object Format
                Self::write_u16(buffer, &mut offset, 0x0001); // Protection Status
                Self::write_u32(buffer, &mut offset, 0); // Object Compressed Size
                Self::write_u16(buffer, &mut offset, 0x3001); // Thumb Format
                Self::write_u32(buffer, &mut offset, 0); // Thumb Compressed Size
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Bit Depth
                Self::write_u32(buffer, &mut offset, 0x00000000); // Parent Object
                Self::write_u16(buffer, &mut offset, 0x0001); // Association Type
                Self::write_u32(buffer, &mut offset, 0); // Association Description
                Self::write_u32(buffer, &mut offset, 0); // Sequence Number
                Self::write_string(buffer, &mut offset, "SNES"); // Filename
                Self::write_string(buffer, &mut offset, "20250714T173222.0Z"); // Date Created
                Self::write_string(buffer, &mut offset, "20250715T183222.0Z"); // Date Modified
                Self::write_string(buffer, &mut offset, "0"); // Keywords
            }
            0x00000005 => {
                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID
                Self::write_u16(buffer, &mut offset, 0x3000); // Object Format
                Self::write_u16(buffer, &mut offset, 0x0001); // Protection Status
                Self::write_u32(buffer, &mut offset, (0x10000 - 0x8000) * 32); // Object Compressed Size
                Self::write_u16(buffer, &mut offset, 0x3000); // Thumb Format
                Self::write_u32(buffer, &mut offset, 0); // Thumb Compressed Size
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Thumb Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Width
                Self::write_u32(buffer, &mut offset, 0); // Image Pix Height
                Self::write_u32(buffer, &mut offset, 0); // Image Bit Depth
                Self::write_u32(buffer, &mut offset, 0x00000004); // Parent Object
                Self::write_u16(buffer, &mut offset, 0); // Association Type
                Self::write_u32(buffer, &mut offset, 0); // Association Description
                Self::write_u32(buffer, &mut offset, 0); // Sequence Number
                Self::write_string(buffer, &mut offset, "rom.sfc"); // Filename
                Self::write_string(buffer, &mut offset, "20250714T173222.0Z"); // Date Created
                Self::write_string(buffer, &mut offset, "20250715T183222.0Z"); // Date Modified
                Self::write_string(buffer, &mut offset, "0"); // Keywords
            }
            _ => {
                return 0;
            }
        }
        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1008);    // Operation: GetStorageIDs
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    async fn generate_rom_object_response(&mut self, transaction_id: u32, buffer: &mut [u8], console: MsgStartConsole) -> usize {
        let mut offset = 0;
        self.out_channel.send(Msg::Start{console}).await;
        let receiver = self.in_channel.receiver();
        loop {
            match receiver.receive().await {
                Msg::DumpSetupData {rom_size} => {
                    Self::write_u32(buffer, &mut offset, rom_size + 12);
                    Self::write_u16(buffer, &mut offset, 2);         // ContainerType: Data
                    Self::write_u16(buffer, &mut offset, 0x1009);    // Operation: GetObject
                    Self::write_u32(buffer, &mut offset, transaction_id);
                },
                Msg::Data {data, length} => {
                    let buffer_write_size = core::cmp::min(length, self.max_packet_size() - 1 - offset);
                    Self::write_buffer(buffer, &mut offset, &data[..buffer_write_size]);
                    if offset == self.max_packet_size() - 1 {
                        offset = 0;
                        match self.write_packet(&buffer[..self.max_packet_size() - 1]).await {
                            Ok(_) => {
                                if buffer_write_size != length {
                                    Self::write_buffer(buffer, &mut offset, &data[buffer_write_size..]);
                                }
                            }
                            _ => {
                                // Allow the USB stack some breathing room; not strictly required
                                // but avoids busy‑looping if the host stalls communication.
                                Timer::after_millis(1).await;
                                break;
                            }
                        }
                    }
                },
                Msg::End => {
                    if offset > 0 {
                        match self.write_packet(&buffer[..offset]).await {
                            Ok(_) => {},
                            _ => {
                                // Allow the USB stack some breathing room; not strictly required
                                // but avoids busy‑looping if the host stalls communication.
                                Timer::after_millis(1).await;
                            }
                        }
                    }
                    if offset % 64 == 0 {
                        match self.write_packet(&[]).await {
                            Ok(_) => {},
                            _ => {
                                // Allow the USB stack some breathing room; not strictly required
                                // but avoids busy‑looping if the host stalls communication.
                                Timer::after_millis(1).await;
                            }
                        }
                    }
                    break;
                },
                _ => {}
            }
        }

        0
    }

    fn generate_config_json_object_response(&mut self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let mut offset = 12;
        Self::write_buffer(buffer, &mut offset, &self.configuration_file[0..self.configuration_file_size]); // File content

        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1009);    // Operation: GetStorageIDs
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    async fn generate_object_response<'a>(&mut self, transaction_id: u32, buffer: &mut [u8], cmd: &PtpCommand<'a>) -> usize {
        let object_handle= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        match object_handle {
            0x00000002 => {
                self.generate_rom_object_response(transaction_id, buffer, MsgStartConsole::Nes).await
            }
            0x00000003 => {
                self.generate_config_json_object_response(transaction_id, buffer)
            }
            0x00000005 => {
                self.generate_rom_object_response(transaction_id, buffer, MsgStartConsole::Snes).await
            }
            _ => {
                0
            }
        }
    }

    fn generate_delete_object_response<'a>(&mut self, cmd: &PtpCommand<'a>) -> usize {
        let object_id= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        if object_id == 0x00000003 || object_id == 0xFFFFFFFF {
            self.configuration_file_deleted = true;
        }
        0
    }

    async fn generate_send_object_info_response<'a>(&mut self, buffer: &mut [u8], cmd: &PtpCommand<'a>) -> usize {
        let storage_id= u32::from_le_bytes(cmd.payload[0..4].try_into().unwrap());
        let parent_id= u32::from_le_bytes(cmd.payload[4..8].try_into().unwrap());
        if storage_id != 0x00010001 && parent_id != 0x00000001 {
            return 0;
        }

        // Read one USB bulk packet from the host.
        let _ = self.read_packet(&mut buffer[0..64]).await;
        let len = match self.read_packet(&mut buffer[64..128]).await {
            Ok(n) if n > 0 => {
                match self.parse_mtp_command(&buffer, MtpContainerType::Data) {
                    Ok(cmd) => {
                        let command_result = match cmd.op_code {
                            0x100c => {
                                let object_format = u16::from_le_bytes(cmd.payload[4..6].try_into().unwrap());
                                let object_compressed_size = u32::from_le_bytes(cmd.payload[8..12].try_into().unwrap());
                                let parent_object=u32::from_le_bytes(cmd.payload[38..42].try_into().unwrap());
                                let association_type=u16::from_le_bytes(cmd.payload[42..44].try_into().unwrap());
                                let association_description=u32::from_le_bytes(cmd.payload[44..48].try_into().unwrap());
                                let filename_length = cmd.payload[52] as usize -1;
                                let filename = &cmd.payload[53..53+filename_length*2];
                                if object_format != 0x3000 {
                                    Err(MtpCommandError::InvalidObjectFormatCode)
                                } else if object_compressed_size as usize > self.configuration_file.len()  {
                                    Err(MtpCommandError::ObjectTooLarge)
                                } else if parent_object != 0x00000001 {
                                    Err(MtpCommandError::InvalidParentObject)
                                } else if association_type != 0 {
                                    Err(MtpCommandError::OperationNotSupported)
                                } else if association_description != 0 {
                                    Err(MtpCommandError::OperationNotSupported)
                                } else if filename_length != "config.json".len() ||
                                    filename.chunks_exact(2)
                                        .map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()))
                                        .zip("config.json".encode_utf16().chain(iter::repeat(0))) // evitiamo panic se lunghezze diverse
                                        .any(|(a, b)| a != b){
                                    Err(MtpCommandError::OperationNotSupported)
                                } else {
                                    Ok(())
                                }
                            }
                            _ => {Err(MtpCommandError::OperationNotSupported)},
                        };
                        match command_result {
                            Ok(()) => {
                                let mut offset = self.generate_ok_response_block(cmd.transaction_id, buffer);
                                Self::write_u32(buffer, &mut offset, 0x00010001); // StorageID in which the object will be stored
                                Self::write_u32(buffer, &mut offset, 0x00000001);// Parent ObjectHandle in which the object will be stored
                                Self::write_u32(buffer, &mut offset, 0x00000003); // Reserved ObjectHandle for the incoming object
                                let length = offset.to_le_bytes();
                                buffer[0..4].copy_from_slice(&length);
                                offset
                            },
                            Err(error) => {self.generate_error_response_block(cmd.transaction_id, buffer, error)},
                        }
                    }
                    _ => {
                        0
                    }
                }
            }
            _ => {
                // Allow the USB stack some breathing room; not strictly required
                // but avoids busy‑looping if the host stalls communication.
                Timer::after_millis(1).await;
                0
            }
        };
        let mut offset = 0;
        while offset < len {
            let end = core::cmp::min(offset + self.max_packet_size(), len);
            let chunk = &buffer[offset..end];
            match self.write_packet(&chunk).await {
                _ => {
                    // Allow the USB stack some breathing room; not strictly required
                    // but avoids busy‑looping if the host stalls communication.
                    Timer::after_millis(1).await;
                }
            }
            offset = end;
        }
        0
    }

    async fn generate_send_object_response(&mut self, buffer: &mut [u8]) -> usize {
        let _ = self.read_packet(&mut buffer[0..64]).await;
        match self.read_packet(&mut buffer[64..128]).await {
            Ok(n) if n > 0 => {
                match self.parse_mtp_command(&buffer, MtpContainerType::Data) {
                    Ok(cmd) => {
                        match cmd.op_code {
                            0x100d => {
                                self.configuration_file.fill(0);
                                self.configuration_file_size = core::cmp::min(cmd.payload.len(), self.configuration_file.len());
                                self.configuration_file[..self.configuration_file_size].copy_from_slice(&cmd.payload[..self.configuration_file_size]);
                                match serde_json_core::from_slice::<DumperConfig>(&self.configuration_file[..self.configuration_file_size]) {
                                    Ok((config, _)) => {
                                        self.send_updated_dumper_config(&config).await;
                                    }
                                    _ => {}
                                };
                            }
                            _ => {}
                        };
                    }
                    _ => {}
                };
            }
            _ => {}
        };
        0
    }

    async fn write_response_buffer(&mut self, buf: &[u8], len: usize) {
        let mut offset = 0;
        while offset < len {
            let end = core::cmp::min(offset + self.max_packet_size(), len);
            let chunk = &buf[offset..end];
            match self.write_packet(&chunk).await {
                Ok(_) => {
                }
                _ => {
                    // Allow the USB stack some breathing room; not strictly required
                    // but avoids busy‑looping if the host stalls communication.
                    Timer::after_millis(1).await;
                }
            }
            offset = end;
        }
        if offset > 0 && offset % 64 == 0 {
            match self.write_packet(&[]).await {
                _ => {
                    // Allow the USB stack some breathing room; not strictly required
                    // but avoids busy‑looping if the host stalls communication.
                    Timer::after_millis(1).await;
                }
            }
        }
    }

    pub async fn handle_response<'a>(&mut self, cmd: PtpCommand<'a>) {
        let mut buf = [0u8; 1024];

        // Data block
        let mut len;
        match cmd.op_code {
            0x1001 => {
                len = self.generate_device_info_response(cmd.transaction_id, &mut buf);
            }
            0x1004 => {
                len = self.generate_storage_id_response(cmd.transaction_id, &mut buf);
            }
            0x1005 => {
                len = self.generate_storage_info_response(cmd.transaction_id, &mut buf, &cmd);
            }
            0x1007 => {
                len = self.generate_object_handles_response(cmd.transaction_id, &mut buf, &cmd);
            }
            0x1008 => {
                len = self.generate_object_info_response(cmd.transaction_id, &mut buf, &cmd);
            }
            0x1009 => {
                len = self.generate_object_response(cmd.transaction_id, &mut buf, &cmd).await;
            }
            0x100b => {
                len = self.generate_delete_object_response(&cmd);
            }
            0x100c => {
                len = self.generate_send_object_info_response(&mut buf, &cmd).await;
            }
            0x100d => {
                len = self.generate_send_object_response(&mut buf).await;
            }
            _ => {
                len = 0;
            }
        }
        if len > 0 {
            self.write_response_buffer(&buf, len).await;
        }

        // Response block
        match cmd.op_code {
            0x1001 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1002 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1003 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1004 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1005 => {
                if len == 0 {
                    len = self.generate_error_response_block(cmd.transaction_id, &mut buf, MtpCommandError::StoreNotAvailable);
                } else {
                    len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
                }
            }
            0x1007 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1008 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x1009 => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x100b => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            0x100d => {
                len = self.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
            _ => {
                len = 0;
            }
        }
        let mut offset = 0;
        while offset < len {
            let end = core::cmp::min(offset + self.max_packet_size(), len);
            let chunk = &buf[offset..end];
            match self.write_packet(&chunk).await {
                _ => {
                    // Allow the USB stack some breathing room; not strictly required
                    // but avoids busy‑looping if the host stalls communication.
                    Timer::after_millis(1).await;
                }
            }
            offset = end;
        }
    }

    async fn send_updated_dumper_config(&mut self, dumper_config: &DumperConfig) {
        let mut field = [0u8;Msg::DUMP_SETUP_DATA_CHANGED_LENGTH];
        let mut value = [0u8;Msg::DUMP_SETUP_DATA_CHANGED_LENGTH];

        field[.."mapper".len()].copy_from_slice("mapper".as_bytes());
        value[..1].copy_from_slice(&[dumper_config.mapper]);
        self.out_channel.send(Msg::DumpSetupDataChanged { field, value }).await;
        field.fill(0);
        value.fill(0);
        field[.."prgsize".len()].copy_from_slice("prgsize".as_bytes());
        value[..1].copy_from_slice(&[dumper_config.prgsize]);
        self.out_channel.send(Msg::DumpSetupDataChanged { field, value }).await;
        field.fill(0);
        value.fill(0);
        field[.."chrsize".len()].copy_from_slice("chrsize".as_bytes());
        value[..1].copy_from_slice(&[dumper_config.chrsize]);
        self.out_channel.send(Msg::DumpSetupDataChanged { field, value }).await;
        field.fill(0);
        value.fill(0);
        field[.."prg".len()].copy_from_slice("prg".as_bytes());
        value[..2].copy_from_slice(&dumper_config.prg.to_ne_bytes());
        self.out_channel.send(Msg::DumpSetupDataChanged { field, value }).await;
        field.fill(0);
        value.fill(0);
        field[.."chr".len()].copy_from_slice("chr".as_bytes());
        value[..2].copy_from_slice(&dumper_config.chr.to_ne_bytes());
        self.out_channel.send(Msg::DumpSetupDataChanged { field, value }).await;
    }
}