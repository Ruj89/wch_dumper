#![no_std]
#![no_main]

use panic_halt as _;
use core::{cell::UnsafeCell, mem::MaybeUninit};
use ch32_hal::usb::EndpointDataBuffer;
use ch32_hal::otg_fs::{self, Driver};
use ch32_hal::{self as hal, bind_interrupts, peripherals, Config};
use ch32_hal::peripherals::OTG_FS;
use embassy_executor::{task, Spawner};
use embassy_usb::{Builder, UsbDevice};
use embassy_time::Timer;

mod usb;

use crate::usb::mtp::MtpClass;

const ENDPOINT_COUNT: usize = 4;

bind_interrupts!(struct Irq { 
    OTG_FS => otg_fs::InterruptHandler<peripherals::OTG_FS>;
});

// ────────────────────────────────────────────────────────────────────────────────
// Wrapper generico: contiene un UnsafeCell ma lo dichiara Sync
// ────────────────────────────────────────────────────────────────────────────────
#[repr(transparent)]
pub struct StaticCell<T>(UnsafeCell<T>);

unsafe impl<T> Sync for StaticCell<T> {}

impl<T> StaticCell<MaybeUninit<T>> {
    pub unsafe fn init(&self, val: T) -> &'static mut T {
        let ptr = self.0.get();
        unsafe {
            if (*ptr).as_ptr().is_null() {
                (*ptr).write(val);
            }
            &mut *(*ptr).assume_init_mut()
        }
    }
}

static EP_BUFFERS: StaticCell<MaybeUninit<[EndpointDataBuffer; ENDPOINT_COUNT]>> =
    StaticCell(UnsafeCell::new(MaybeUninit::uninit()));
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static BOS_DESCRIPTOR   : StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static MSOS_DESCRIPTOR  : StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static CONTROL_BUF      : StaticCell<[u8;  64]> = StaticCell(UnsafeCell::new([0;  64]));

#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(spawner: Spawner) -> ! {
    // setup clocks
    let cfg = Config {
        rcc: ch32_hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    };
    let p = hal::init(cfg);

    let buffer = unsafe {
        EP_BUFFERS.init(core::array::from_fn(|_| EndpointDataBuffer::default()))
    };
    let driver = Driver::new(p.OTG_FS, p.PA12, p.PA11, buffer);

    // Create embassy-usb Config
    let mut config = embassy_usb::Config::new(0x6666, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB MTP Demo");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0x00;
    config.device_sub_class = 0x00;
    config.device_protocol = 0x00;
    config.composite_with_iads = false;
    
    let mut builder = Builder::new(
        driver,
        config,
        unsafe { &mut *CONFIG_DESCRIPTOR.0.get() },
        unsafe { &mut *BOS_DESCRIPTOR   .0.get() },
        unsafe { &mut *MSOS_DESCRIPTOR  .0.get() },
        unsafe { &mut *CONTROL_BUF      .0.get() },
    );

    // The maximum packet size MUST be 8/16/32/64 on full‑speed.
    const MAX_PACKET_SIZE: u16 = 64;
    let mtp_class = MtpClass::new(&mut builder, MAX_PACKET_SIZE);

    // Build the final `UsbDevice` which owns the internal state.
    let usb_device = builder.build();

    // ──────────────────────────────────────────────────────────────────────────────
    // Spawn async tasks
    // ──────────────────────────────────────────────────────────────────────────────
    spawner.spawn(usb_device_task(usb_device)).unwrap();
    spawner.spawn(mtp_echo_task(mtp_class)).unwrap();

    // The main task can now sleep forever; all work happens in the spawned tasks.
    loop {
        core::future::pending::<()>().await;
    }
}

/// Task that drives the USB device state machine.
#[task]
async fn usb_device_task(mut device: UsbDevice<'static, Driver<'static, OTG_FS, ENDPOINT_COUNT>>) {
    device.run().await;
}

/// Very small demo: wait for the host to open the interface and then echo what we
/// receive back to the host.
#[task]
async fn mtp_echo_task(mut mtp: MtpClass<'static, Driver<'static, OTG_FS, ENDPOINT_COUNT>>) {
    // Block until the host has configured the interface.
    mtp.wait_connection().await;

    let mut buf = [0u8; 64];
    loop {
        // Read one USB bulk packet from the host.
        match mtp.read_packet(&mut buf).await {
            Ok(n) if n > 0 => {
                if let Some(cmd) = mtp.parse_mtp_command(&buf) {
                    let len;
                    //match cmd.op_code {
                    //    0x1001 => {
                    //        len = mtp.generate_device_info_response(cmd.transaction_id, &mut buf);
                    //    }
                    //    0x1002 => {
                            len = mtp.generate_open_session_response(cmd.transaction_id, &mut buf);
                    //    }
                    //    _ => {
                    //        len = 0;
                    //    }
                    //}
                    if len > 0 {
                        match mtp.write_packet(&buf[..len]).await {
                            Ok(_) => {
                                Timer::after_millis(1).await;
                            }
                            _ => {
                                // Allow the USB stack some breathing room; not strictly required
                                // but avoids busy‑looping if the host stalls communication.
                                Timer::after_millis(1).await;
                            }
                        }
                    }
                }
            }
            _ => {
                // Allow the USB stack some breathing room; not strictly required
                // but avoids busy‑looping if the host stalls communication.
                Timer::after_millis(1).await;
            }
        }
    }
}
