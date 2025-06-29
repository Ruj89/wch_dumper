//! CDC-ACM class implementation, aka Serial over USB.
#![no_std]

use embassy_usb::driver::{Driver, EndpointError, Endpoint, EndpointIn, EndpointOut};
use embassy_usb::types::InterfaceNumber;
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

/// Packet level implementation of a CDC-ACM serial port.
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
    _data_if: InterfaceNumber,
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
        let data_if = iface.interface_number();
        let mut alt = iface.alt_setting(USB_CLASS_MTP, MTP_SUBCLASS, MTP_PROTOCOL, None);
        let read_ep = alt.endpoint_bulk_out(max_packet_size);
        let write_ep = alt.endpoint_bulk_in(max_packet_size);
        let comm_ep = alt.endpoint_interrupt_in(8, 255);

        drop(func);

        MtpClass {
            _comm_ep: comm_ep,
            _data_if: data_if,
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
        self.write_ep.write(data).await
    }

    /// Reads a single packet from the OUT endpoint.
    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    /// Waits for the USB host to enable this interface
    pub async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
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

/// CDC ACM class packet sender.
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

impl<'d, D: Driver<'d>> embedded_io_async::ErrorType for Sender<'d, D> {
    type Error = EndpointError;
}

impl<'d, D: Driver<'d>> embedded_io_async::Write for Sender<'d, D> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let len = core::cmp::min(buf.len(), self.max_packet_size() as usize);
        self.write_packet(&buf[..len]).await?;
        Ok(len)
    }
}

/// CDC ACM class packet receiver.
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

    /// Turn the `Receiver` into a [`BufferedReceiver`].
    ///
    /// The supplied buffer must be large enough to hold max_packet_size bytes.
    pub fn into_buffered(self, buf: &'d mut [u8]) -> BufferedReceiver<'d, D> {
        BufferedReceiver {
            receiver: self,
            buffer: buf,
            start: 0,
            end: 0,
        }
    }
}

/// CDC ACM class buffered receiver.
///
/// It is a requirement of the [`embedded_io_async::Read`] trait that arbitrarily small lengths of
/// data can be read from the stream. The [`Receiver`] can only read full packets at a time. The
/// `BufferedReceiver` instead buffers a single packet if the caller does not read all of the data,
/// so that the remaining data can be returned in subsequent calls.
///
/// If you have no requirement to use the [`embedded_io_async::Read`] trait or to read a data length
/// less than the packet length, then it is more efficient to use the [`Receiver`] directly.
///
/// You can obtain a `BufferedReceiver` with [`Receiver::into_buffered`].
///
/// [`embedded_io_async::Read`]: https://docs.rs/embedded-io-async/latest/embedded_io_async/trait.Read.html
pub struct BufferedReceiver<'d, D: Driver<'d>> {
    receiver: Receiver<'d, D>,
    buffer: &'d mut [u8],
    start: usize,
    end: usize,
}

impl<'d, D: Driver<'d>> BufferedReceiver<'d, D> {
    fn read_from_buffer(&mut self, buf: &mut [u8]) -> usize {
        let available = &self.buffer[self.start..self.end];
        let len = core::cmp::min(available.len(), buf.len());
        buf[..len].copy_from_slice(&self.buffer[..len]);
        self.start += len;
        len
    }

    /// Waits for the USB host to enable this interface
    pub async fn wait_connection(&mut self) {
        self.receiver.wait_connection().await;
    }
}

impl<'d, D: Driver<'d>> embedded_io_async::ErrorType for BufferedReceiver<'d, D> {
    type Error = EndpointError;
}

impl<'d, D: Driver<'d>> embedded_io_async::Read for BufferedReceiver<'d, D> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // If there is a buffered packet, return data from that first
        if self.start != self.end {
            return Ok(self.read_from_buffer(buf));
        }

        // If the caller's buffer is large enough to contain an entire packet, read directly into
        // that instead of buffering the packet internally.
        if buf.len() > self.receiver.max_packet_size() as usize {
            return self.receiver.read_packet(buf).await;
        }

        // Otherwise read a packet into the internal buffer, and return some of it to the caller
        self.start = 0;
        self.end = self.receiver.read_packet(&mut self.buffer).await?;
        return Ok(self.read_from_buffer(buf));
    }
}