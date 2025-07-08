//! MTP class implementation.

use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder};

/// This should be used as `device_class` when building the `UsbDevice`.
const USB_CLASS_MTP: u8 = 0x06;
const MTP_SUBCLASS: u8 = 0x01;
const MTP_PROTOCOL: u8 = 0x01;

const CS_INTERFACE: u8 = 0x24;
const CDC_TYPE_HEADER: u8 = 0x00;
const CDC_TYPE_ACM: u8 = 0x02;
const CDC_TYPE_UNION: u8 = 0x06;

const REQ_SEND_ENCAPSULATED_COMMAND: u8 = 0x00;
#[allow(unused)]
const REQ_GET_ENCAPSULATED_COMMAND: u8 = 0x01;
const REQ_SET_LINE_CODING: u8 = 0x20;
const REQ_GET_LINE_CODING: u8 = 0x21;
const REQ_SET_CONTROL_LINE_STATE: u8 = 0x22;

#[derive(Debug)]
pub struct PtpCommand {
    pub op_code: u16,
    pub transaction_id: u32,
    //pub session_id: u32,
}

/// Errors returned by [`MtpClass::parse_mtp_command`]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MtpError {
    CannotParseHeader,
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
    _comm_ep: D::EndpointIn,
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}

impl<'d, D: Driver<'d>> MtpClass<'d, D> {
    /// Creates a new MtpClass with the provided UsbBus and `max_packet_size` in bytes. For
    /// full-speed devices, `max_packet_size` has to be one of 8, 16, 32 or 64.
    pub fn new(builder: &mut Builder<'d, D>, max_packet_size: u16) -> Self {
        assert!(builder.control_buf_len() >= 7);

        let mut func = builder.function(0x00, 0x00, 0x00);
        let mut iface = func.interface();
        let mut alt = iface.alt_setting(USB_CLASS_MTP, MTP_SUBCLASS, MTP_PROTOCOL, None);
        let read_ep = alt.endpoint_bulk_out(max_packet_size);
        let write_ep = alt.endpoint_bulk_in(max_packet_size);
        let comm_ep = alt.endpoint_interrupt_in(8, 255);

        drop(func);

        MtpClass {
            _comm_ep: comm_ep,
            read_ep,
            write_ep,
        }
    }

    /// Gets the maximum packet size in bytes.
    pub fn max_packet_size(&self) -> u16 {
        // The size is the same for both endpoints.
        self.read_ep.info().max_packet_size
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

    pub fn parse_mtp_command(&self, buf: &[u8]) -> Result<PtpCommand, MtpError> {
        let length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if length < 12 {
            return Err(MtpError::CannotParseHeader);
        }
        let packet_type = u16::from_le_bytes([buf[4], buf[5]]);
        let op_code = u16::from_le_bytes([buf[6], buf[7]]);
        let transaction_id = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        //let session_id = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);

        /*if length != 16 || packet_type != 0x0001 || op_code != 0x1002 {
            return None;
        }*/

        Ok(PtpCommand {
            op_code,
            transaction_id,
            //session_id,
        })
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

    // PTP string format: len (u8), UTF-16LE chars, 0x0000 terminator
    fn write_ptp_string(buf: &mut [u8], mut offset: usize, s: &str) -> usize {
        let mut len = 1;
        let start = offset;

        buf[offset] = (s.len() + 1) as u8; // total chars incl. null
        offset += 1;

        for c in s.encode_utf16() {
            let b = c.to_le_bytes();
            buf[offset] = b[0];
            buf[offset + 1] = b[1];
            offset += 2;
            len += 1;
        }

        // null terminator UTF-16
        buf[offset] = 0;
        buf[offset + 1] = 0;
        len += 1;

        offset + 2 - start
    }

    pub fn generate_open_session_response(&self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let length = 12u32.to_le_bytes();
        let packet_type = 0x0003u16.to_le_bytes();       // Response Block
        let response_code = 0x2001u16.to_le_bytes();     // OK
        let tx_id = transaction_id.to_le_bytes();

        buffer[0..4].copy_from_slice(&length);
        buffer[4..6].copy_from_slice(&packet_type);
        buffer[6..8].copy_from_slice(&response_code);
        buffer[8..12].copy_from_slice(&tx_id);

        12
    }

    pub fn generate_device_info_response(&self, transaction_id: u32, buffer: &mut [u8]) -> usize {
        let mut offset = 12;
        Self::write_u16(buffer, &mut offset, 0x0100); // StandardVersion
        Self::write_u32(buffer, &mut offset, 6); // VendorExtensionID = 6 (Microsoft)
        Self::write_u16(buffer, &mut offset, 100);  // VendorExtensionVersion
        offset += Self::write_ptp_string(buffer, offset, "microsoft.com: 1.0;"); // VendorExtensionDesc
        Self::write_u16(buffer, &mut offset, 0); // FunctionalMode
        Self::write_u32(buffer, &mut offset, 1); // NumOperationsSupported
        Self::write_u16(buffer, &mut offset, 0x1001); // GetDeviceInfo
        Self::write_u32(buffer, &mut offset, 0); // EventsSupported = empty
        Self::write_u32(buffer, &mut offset, 0); // DevicePropertiesSupported = empty
        Self::write_u32(buffer, &mut offset, 0); // CaptureFormats = empty
        Self::write_u32(buffer, &mut offset, 0); // PlaybackFormats = empty
        offset += Self::write_ptp_string(buffer, offset, "MyCompany"); // Manufacturer
        offset += Self::write_ptp_string(buffer, offset, "MTP Device"); // Model
        offset += Self::write_ptp_string(buffer, offset, "1.0"); // DeviceVersion
        offset += Self::write_ptp_string(buffer, offset, "12345678"); // SerialNumber
        let total_len = offset as u32;
        Self::write_u32(buffer, &mut 0, total_len);
        Self::write_u16(buffer, &mut 4, 2);         // ContainerType: Data
        Self::write_u16(buffer, &mut 6, 0x1001);    // Operation: GetDeviceInfo
        Self::write_u32(buffer, &mut 8, transaction_id);

        offset
    }

    /// Split the class into a sender and receiver.
    ///
    /// This allows concurrently sending and receiving packets from separate tasks.
    pub fn split(self) -> (Sender<'d, D>, Receiver<'d, D>) {
        (
            Sender {
                write_ep: self.write_ep,
            },
            Receiver {
                read_ep: self.read_ep,
            },
        )
    }
}

/// MTP class packet sender.
///
/// You can obtain a `Sender` with [`MtpClass::split`]
pub struct Sender<'d, D: Driver<'d>> {
    write_ep: D::EndpointIn,
}

impl<'d, D: Driver<'d>> Sender<'d, D> {
    /// Gets the maximum packet size in bytes.
    pub fn max_packet_size(&self) -> u16 {
        // The size is the same for both endpoints.
        self.write_ep.info().max_packet_size
    }

    /// Writes a single packet into the IN endpoint.
    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }

    /// Waits for the USB host to enable this interface
    pub async fn wait_connection(&mut self) {
        self.write_ep.wait_enabled().await;
    }
}

/// MTP class packet receiver.
///
/// You can obtain a `Receiver` with [`MtpClass::split`]
pub struct Receiver<'d, D: Driver<'d>> {
    read_ep: D::EndpointOut,
}

impl<'d, D: Driver<'d>> Receiver<'d, D> {
    /// Gets the maximum packet size in bytes.
    pub fn max_packet_size(&self) -> u16 {
        // The size is the same for both endpoints.
        self.read_ep.info().max_packet_size
    }

    /// Reads a single packet from the OUT endpoint.
    /// Must be called with a buffer large enough to hold max_packet_size bytes.
    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    /// Waits for the USB host to enable this interface
    pub async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }
}