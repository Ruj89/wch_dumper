#![no_std]
#![no_main]

use panic_halt as _;
use ch32_hal::usb::EndpointDataBuffer;
use ch32_hal::otg_fs::{self, Driver};
use ch32_hal::{self as hal, bind_interrupts, peripherals, Config};
use ch32_hal::peripherals::OTG_FS;
use embassy_executor::{task, Spawner};
use embassy_usb::{Builder, UsbDevice};
use embassy_time::Timer;

mod usb;

use crate::usb::mtp::MtpClass;

bind_interrupts!(struct Irq {
    OTG_FS => otg_fs::InterruptHandler<peripherals::OTG_FS>;
});
static mut EP_BUFFERS: MaybeUninit<[EndpointDataBuffer; EP_COUNT]> = MaybeUninit::uninit();


#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(spawner: Spawner) -> ! {
    // setup clocks
    let cfg = Config {
        rcc: ch32_hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    };
    let p = hal::init(cfg);

    /* USB DRIVER SECION */    
    let buffer = unsafe {
        // Safety:
        // 1. Siamo in `main`, quindi viene eseguito una sola volta.
        // 2. Dopo questa chiamata passeremo l’unico &mut a `Driver::new`,
        //    che lo userà per tutta la durata del programma: nessun alias.
        EP_BUFFERS.write(core::array::from_fn(|_| EndpointDataBuffer::default()))
    };
    let driver = Driver::new(p.OTG_FS, p.PA12, p.PA11, &mut buffer);

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

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    // You can also add a Microsoft OS descriptor.
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    
    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
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
async fn usb_device_task(mut device: UsbDevice<'static, Driver<'static, OTG_FS, 4>>) {
    device.run().await;
}

/// Very small demo: wait for the host to open the interface and then echo what we
/// receive back to the host.
#[task]
async fn mtp_echo_task(mut mtp: MtpClass<'static, Driver<'static, OTG_FS, 4>>) {
    // Block until the host has configured the interface.
    mtp.wait_connection().await;

    // Send a greeting so that the host sees *something* on connect.
    let _ = mtp.write_packet(b"Hello from Rust MTP!\r\n").await;

    let mut buf = [0u8; 64];
    loop {
        // Read one USB bulk packet from the host.
        match mtp.read_packet(&mut buf).await {
            Ok(n) if n > 0 => {
                // Echo the data back.
                let _ = mtp.write_packet(&buf[..n]).await;
            }
            _ => {
                // Allow the USB stack some breathing room; not strictly required
                // but avoids busy‑looping if the host stalls communication.
                Timer::after_millis(1).await;
            }
        }
    }
}
