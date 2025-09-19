#![no_std]
#![no_main]

use ch32_hal::gpio::{Flex, Input, Pull};
use panic_halt as _;
use core::{cell::UnsafeCell, mem::MaybeUninit};
use ch32_hal::usb::EndpointDataBuffer;
use ch32_hal::otg_fs::{self, Driver};
use ch32_hal::{self as hal, bind_interrupts, peripherals, Config};
use ch32_hal::peripherals::OTG_FS;
use embassy_executor::{task, Spawner};
use embassy_usb::{Builder, UsbDevice};
use embassy_time::Timer;
use hal::gpio::{Level, Output};

mod usb;

use crate::usb::mtp::{MtpClass, PtpCommand};

const ENDPOINT_COUNT: usize = 14;

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
static CRC32            : StaticCell<[u8; 512]> = StaticCell(UnsafeCell::new([0;  512]));
static CRC32_MMC3       : StaticCell<[u8; 512]> = StaticCell(UnsafeCell::new([0;  512]));

#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(spawner: Spawner) -> ! {
    // setup clocks
    let cfg = Config {
        rcc: ch32_hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    };
    let p = hal::init(cfg);
    
    let mut m2 = Output::new(p.PB12, Level::High, Default::default());
    let mut pgr_ce = Output::new(p.PE1, Level::High, Default::default());
    let mut chr_wr = Output::new(p.PB10, Level::High, Default::default());
    let mut ciram_ce = Input::new(p.PE0, Pull::Up);
    let mut chr_rd = Output::new(p.PB7, Level::High, Default::default());
    let mut irq = Input::new(p.PE6, Pull::Up);
    let mut prg_rw = Output::new(p.PD15, Level::High, Default::default());
    
    let mut a = [
        Output::new(p.PD0, Level::Low, Default::default()),
        Output::new(p.PC12, Level::Low, Default::default()),
        Output::new(p.PC11, Level::Low, Default::default()),
        Output::new(p.PC10, Level::Low, Default::default()),
        Output::new(p.PA15, Level::Low, Default::default()),
        Output::new(p.PE3, Level::Low, Default::default()),
        Output::new(p.PE4, Level::Low, Default::default()),
        Output::new(p.PB13, Level::Low, Default::default()),
        Output::new(p.PB15, Level::Low, Default::default()),
        Output::new(p.PD4, Level::Low, Default::default()),
        Output::new(p.PA8, Level::Low, Default::default()),
        Output::new(p.PD3, Level::Low, Default::default()),
        Output::new(p.PA9, Level::Low, Default::default()),
        Output::new(p.PD2, Level::Low, Default::default()),
        Output::new(p.PA10, Level::Low, Default::default()),
        Output::new(p.PB11, Level::High, Default::default()),
    ];

    let mut ciram_a10 = Input::new(p.PD6, Pull::Up);

    let mut d = [
        Flex::new(p.PE5),
        Flex::new(p.PD13),
        Flex::new(p.PB6),
        Flex::new(p.PB14),
        Flex::new(p.PD8),
        Flex::new(p.PD9),
        Flex::new(p.PD10),
        Flex::new(p.PD11)
    ];
    for dpin in &mut d {
        dpin.set_as_input(Pull::Up);
    }
    set_address(&mut a, 0);
    

    let crc32 =    unsafe { &mut *CRC32.0.get() };
    let crc32_mmc3 =    unsafe { &mut *CRC32_MMC3.0.get() };

    for c in 0..512 {
        crc32[c] = read_prg_byte(u16::try_from(0x8000 + c).expect("address overflow"),&mut (&mut a, &mut d, &mut prg_rw, &mut pgr_ce, &mut m2)).await;
        crc32_mmc3[c] = read_prg_byte(u16::try_from(0xE000 + c).expect("address overflow"),&mut (&mut a, &mut d, &mut prg_rw, &mut pgr_ce, &mut m2)).await;
    }

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
    let mtp_class = MtpClass::new(
        &mut builder, 
        MAX_PACKET_SIZE, 
        unsafe { &mut *CRC32.0.get() },
        unsafe { &mut *CRC32_MMC3.0.get() });

    // Build the final `UsbDevice` which owns the internal state.
    let usb_device = builder.build();

    // ──────────────────────────────────────────────────────────────────────────────
    // Spawn async tasks
    // ──────────────────────────────────────────────────────────────────────────────
    spawner.spawn(mtp_echo_task(mtp_class)).unwrap();
    spawner.spawn(usb_device_task(usb_device)).unwrap();

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
                match mtp.parse_mtp_command(&buf) {
                    Ok(cmd) => {
                        handle_response(&mut mtp, cmd).await;
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

async fn handle_response<'a>(mtp: &mut MtpClass<'static, Driver<'static, OTG_FS, ENDPOINT_COUNT>>, cmd: PtpCommand<'a>) {
    let mut buf = [0u8; 1024+12]; //TODO: Remove when 0x1009 has been fixed

    // Data block
    let mut len;
    match cmd.op_code {
        0x1001 => {
            len = mtp.generate_device_info_response(cmd.transaction_id, &mut buf);
        }
        0x1004 => {
            len = mtp.generate_storage_id_response(cmd.transaction_id, &mut buf);
        }
        0x1005 => {
            len = mtp.generate_storage_info_response(cmd.transaction_id, &mut buf, &cmd);
        }
        0x1007 => {
            len = mtp.generate_object_handles_response(cmd.transaction_id, &mut buf, &cmd);
        }
        0x1008 => {
            len = mtp.generate_object_info_response(cmd.transaction_id, &mut buf, &cmd);
        }
        0x1009 => {
            len = mtp.generate_object_response(cmd.transaction_id, &mut buf, &cmd);
        }
        _ => {
            len = 0;
        }
    }
    let mut offset = 0;
    while offset < len {
        let end = core::cmp::min(offset + mtp.max_packet_size(), len);
        let chunk = &buf[offset..end];
        match mtp.write_packet(&chunk).await {
            Ok(_) => {
                Timer::after_millis(1).await;
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
        match mtp.write_packet(&[]).await {
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

    // Response block
    match cmd.op_code {
        0x1001 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1002 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1003 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1004 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1005 => {
            if len == 0 {
                len = mtp.generate_error_response_block(cmd.transaction_id, &mut buf, 0x2013);
            } else {
                len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
            }
        }
        0x1007 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1008 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        0x1009 => {
            len = mtp.generate_ok_response_block(cmd.transaction_id, &mut buf);
        }
        _ => {
            len = 0;
        }
    }
    let mut offset = 0;
    while offset < len {
        let end = core::cmp::min(offset + mtp.max_packet_size(), len);
        let chunk = &buf[offset..end];
        match mtp.write_packet(&chunk).await {
            Ok(_) => {
                Timer::after_millis(1).await;
            }
            _ => {
                // Allow the USB stack some breathing room; not strictly required
                // but avoids busy‑looping if the host stalls communication.
                Timer::after_millis(1).await;
            }
        }
        offset = end;
    }
}

fn set_address(handler: &mut [Output<'_>; 16], address: u16) {
    let mut values: [Level; 16] = [Level::Low; 16];

    // Prepare values
    for index in 0..handler.len() - 1 {
        values[index] = Level::from((address & (1 << index)) > 0)
    }
    // PPU /A13
    values[handler.len()-1] = Level::from((address & (1 << 13)) == 0);
    
    // Set GPIO values
    for index in 0..handler.len() {
        handler[index].set_level(values[index]); 
    }
}

fn set_read_mode(handler: &mut [Flex<'_>; 8]) {
    for pin in handler.iter_mut() {
        pin.set_as_input(Pull::Up);
    }
}

fn set_write_mode(handler: &mut [Flex<'_>; 8]) {
    for pin in handler.iter_mut() {
        pin.set_as_output(Default::default());
        pin.set_low();
    }
}

fn set_prg_read(handler: &mut Output<'_>){
    handler.set_high();
}

fn set_romsel_low(handler: &mut Output<'_>){
    handler.set_low();
}

fn set_romsel_high(handler: &mut Output<'_>){
    handler.set_high();
}

fn set_romsel(handler: &mut Output<'_>, address: u16) {
  if address & 0x8000 > 0 {
    set_romsel_low(handler);
  } else {    
    set_romsel_high(handler);
  }
}

fn set_phy2_high(handler: &mut Output<'_>){
    handler.set_high();
}

fn set_phy2_low(handler: &mut Output<'_>){
    handler.set_low();
}

fn read_data(handler: &mut [Flex<'_>; 8]) -> u8{
    let mut data = 0;
    for (index, pin) in handler.iter().enumerate() {
        data |= (pin.is_high() as u8) << index; 
    }
    data
}

async fn read_prg_byte(address: u16, handler: &mut (&mut [Output<'_>; 16], &mut [Flex<'_>; 8], &mut Output<'_>, &mut Output<'_>, &mut Output<'_>)) -> u8 {
    set_read_mode(handler.1);
    set_prg_read(handler.2);
    set_romsel_high(handler.3);
    set_address(handler.0, address);
    set_phy2_high(handler.4);
    set_romsel(handler.3, address);
    Timer::after_micros(1).await;
    read_data(handler.1)
}