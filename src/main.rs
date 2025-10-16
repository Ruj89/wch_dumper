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
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[path = "usb/mtp.rs"]
mod mtp;
#[path = "dumper/dumper.rs"]
mod dumper;

use mtp::{MtpClass, MtpContainerType};
use dumper::{DumperClass, Msg, DATA_CHANNEL_SIZE};

const ENDPOINT_COUNT: usize = 14;

bind_interrupts!(struct Irq {
    OTG_FS => otg_fs::InterruptHandler<peripherals::OTG_FS>;
});

static TO_DUMPER_CHANNEL: Channel<CriticalSectionRawMutex, Msg, 1> = Channel::new();
static TO_USB_CHANNEL: Channel<CriticalSectionRawMutex, Msg, 1> = Channel::new();

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
static CONFIG_DESCRIPTOR        : StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static BOS_DESCRIPTOR           : StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static MSOS_DESCRIPTOR          : StaticCell<[u8; 256]> = StaticCell(UnsafeCell::new([0; 256]));
static CONTROL_BUF              : StaticCell<[u8;  64]> = StaticCell(UnsafeCell::new([0;  64]));
static DUMPER_BUF               : StaticCell<[u8;  DATA_CHANNEL_SIZE]> = StaticCell(UnsafeCell::new([0;  DATA_CHANNEL_SIZE]));
static DUMPER_CONFIGURATION_BUF : StaticCell<[u8;1024]> = StaticCell(UnsafeCell::new([0;  1024]));

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
    config.manufacturer = Some("arkHive");
    config.product = Some("MTP Dumper");
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
    let dumper = DumperClass::new(
        p.PB12,
        p.PE1,
        p.PB10,
        p.PE0,
        p.PB7,
        p.PE6,
        p.PD15,
        (
            p.PD0,
            p.PC12,
            p.PC11,
            p.PC10,
            p.PA15,
            p.PE3,
            p.PE4,
            p.PB13,
            p.PB15,
            p.PD4,
            p.PA8,
            p.PD3,
            p.PA9,
            p.PD2,
            p.PA10,
            p.PB11,
        ),
        p.PD6,
        (
            p.PE5,
            p.PD13,
            p.PB6,
            p.PB14,
            p.PD8,
            p.PD9,
            p.PD10,
            p.PD11
        ),
        &TO_DUMPER_CHANNEL,
        &TO_USB_CHANNEL,
        unsafe { &mut *DUMPER_BUF.0.get() },
    );

    let mtp_class = MtpClass::new(
        &mut builder,
        MAX_PACKET_SIZE,
        &TO_USB_CHANNEL,
        &TO_DUMPER_CHANNEL,
        unsafe { &mut *DUMPER_CONFIGURATION_BUF.0.get() },
    );

    // Build the final `UsbDevice` which owns the internal state.
    let usb_device = builder.build();

    // ──────────────────────────────────────────────────────────────────────────────
    // Spawn async tasks
    // ──────────────────────────────────────────────────────────────────────────────
    spawner.spawn(mtp_task(mtp_class)).unwrap();
    spawner.spawn(usb_device_task(usb_device)).unwrap();
    spawner.spawn(rom_read_task(dumper)).unwrap();

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
async fn mtp_task(mut mtp: MtpClass<'static, Driver<'static, OTG_FS, ENDPOINT_COUNT>>) {
    // Block until the host has configured the interface.
    mtp.wait_connection().await;

    let mut buf = [0u8; 64];
    loop {
        // Read one USB bulk packet from the host.
        match mtp.read_packet(&mut buf).await {
            Ok(n) if n > 0 => {
                match mtp.parse_mtp_command(&buf, MtpContainerType::Command) {
                    Ok(cmd) => {
                        mtp.handle_response(cmd).await;
                    }
                    _ => {
                        // TODO: Handle error
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

#[task]
async fn rom_read_task(mut dumper: DumperClass<'static>) {
    dumper.dump().await;
}