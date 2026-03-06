use ch32_hal::Peri;
use ch32_hal::gpio::{Flex, Level, Output, Pin, Pull};
use embassy_time::Timer;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

#[cfg(feature = "nes")]
pub const BYTE_READ_RETRIES: usize = 1;

pub enum MsgStartConsole {
    #[cfg(feature = "nes")] Nes,
    #[cfg(feature = "snes")] Snes,
    #[cfg(feature = "ms")] Sms,
    #[cfg(feature = "md")] Md,
}

impl Msg {
    pub const DATA_CHANNEL_SIZE: usize = 32;
    #[cfg(feature = "nes")] pub const DUMP_SETUP_DATA_CHANGED_LENGTH: usize = Msg::DATA_CHANNEL_SIZE / 2;
}

pub enum Msg {
    Start {
        console: MsgStartConsole
    },
    DumpSetupData {
        rom_size: u32,
    },
    #[cfg(feature = "nes")]
    DumpSetupDataChanged {
        field: [u8;Self::DUMP_SETUP_DATA_CHANGED_LENGTH],
        value: [u8;Self::DUMP_SETUP_DATA_CHANGED_LENGTH],
    },
    Data {
        data: [u8; Msg::DATA_CHANNEL_SIZE],
        length: usize
    },
    End,
}

#[cfg(feature = "nes")]
pub struct DumperConfig {
    pub mapper: u8,
    pub prgsize: u8,
    pub chrsize: u8,
    pub prg: u16, // KB
    pub chr: u16, // KB
    #[cfg(feature = "md")] pub segaSram16bit: bool,
}

#[repr(u8)]
#[cfg(feature = "snes")]
pub enum SnesRomType {
    LO = 0,
    HI = 1,
    SA = 3,
    EX = 4,
}
pub struct DumperClass<'d> {
    m2: Output<'d>,
    pgr_ce: Output<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] chr_wr: Output<'d>,
    ciram_ce: Flex<'d>,
    chr_rd: Output<'d>,
    irq: Flex<'d>,
    prg_rw: Output<'d>,
    a: [Flex<'d>; 16],
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] ciram_a10: Flex<'d>,
    d: [Flex<'d>; 8],
    #[cfg(any(feature = "snes", feature = "md"))] a15: Flex<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] reset: Output<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] cs: Output<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] wr: Output<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] rd: Output<'d>,
    #[cfg(any(feature = "snes"))] refresh: Output<'d>,
    #[cfg(any(feature = "snes", feature = "md"))] expand: Flex<'d>,
    #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] d_snes: [Flex<'d>; 7],
    #[cfg(any(feature = "snes", feature = "md"))] irq_snes: Flex<'d>,
    #[cfg(feature = "md")] asout: Output<'d>,
    #[cfg(feature = "md")] clk: Output<'d>,
    #[cfg(feature = "md")] time: Output<'d>,
    in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    buffer: &'d mut [u8; Msg::DATA_CHANNEL_SIZE],
    #[cfg(feature = "nes")] config: DumperConfig,
}

impl<'d> DumperClass<'d>
{
    pub fn new(
        m2_pin: Peri<'d, impl Pin>,
        pgr_ce_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] chr_wr_pin: Peri<'d, impl Pin>,
        ciram_ce_pin: Peri<'d, impl Pin>,
        chr_rd_pin: Peri<'d, impl Pin>,
        irq_pin: Peri<'d, impl Pin>,
        prg_rw_pin: Peri<'d, impl Pin>,
        a_pins: (
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
        ),
        #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] ciram_a10_pin: Peri<'d, impl Pin>,
        d_pins: (
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
        ),
        #[cfg(any(feature = "snes", feature = "md"))] a15_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] reset_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] cs_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] wr_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] rd_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes"))] refresh_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "md"))] expand_pin: Peri<'d, impl Pin>,
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] d_snes_pins: (
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
            Peri<'d, impl Pin>,
        ),
        #[cfg(any(feature = "snes", feature = "md"))] irq_snes_pin: Peri<'d, impl Pin>,
        #[cfg(feature = "md")]asout_pin: Peri<'d, impl Pin>,
        #[cfg(feature = "md")]clk_pin: Peri<'d, impl Pin>,
        #[cfg(feature = "md")]time_pin: Peri<'d, impl Pin>,
        in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        buffer: &'d mut [u8; Msg::DATA_CHANNEL_SIZE],
    ) -> Self {
        let m2 = Output::new(m2_pin, Level::High, Default::default());
        let pgr_ce = Output::new(pgr_ce_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] let chr_wr = Output::new(chr_wr_pin, Level::High, Default::default());
        let ciram_ce = Flex::new(ciram_ce_pin);
        let chr_rd = Output::new(chr_rd_pin, Level::High, Default::default());
        let irq: Flex<'_> = Flex::new(irq_pin);
        let prg_rw = Output::new(prg_rw_pin, Level::High, Default::default());

        let a = [
            Flex::new(a_pins.0),
            Flex::new(a_pins.1),
            Flex::new(a_pins.2),
            Flex::new(a_pins.3),
            Flex::new(a_pins.4),
            Flex::new(a_pins.5),
            Flex::new(a_pins.6),
            Flex::new(a_pins.7),
            Flex::new(a_pins.8),
            Flex::new(a_pins.9),
            Flex::new(a_pins.10),
            Flex::new(a_pins.11),
            Flex::new(a_pins.12),
            Flex::new(a_pins.13),
            Flex::new(a_pins.14),
            Flex::new(a_pins.15),
        ];

        #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] let ciram_a10 = Flex::new(ciram_a10_pin);

        let d = [
            Flex::new(d_pins.0),
            Flex::new(d_pins.1),
            Flex::new(d_pins.2),
            Flex::new(d_pins.3),
            Flex::new(d_pins.4),
            Flex::new(d_pins.5),
            Flex::new(d_pins.6),
            Flex::new(d_pins.7)
        ];

        #[cfg(any(feature = "snes", feature = "md"))] let a15 = Flex::new(a15_pin);
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] let reset = Output::new(reset_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] let cs = Output::new(cs_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] let wr: Output<'_> = Output::new(wr_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] let rd: Output<'_> = Output::new(rd_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes"))] let refresh = Output::new(refresh_pin, Level::High, Default::default());
        #[cfg(any(feature = "snes", feature = "md"))] let expand = Flex::new(expand_pin);

        #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] let d_snes = [
            Flex::new(d_snes_pins.0),
            Flex::new(d_snes_pins.1),
            Flex::new(d_snes_pins.2),
            Flex::new(d_snes_pins.3),
            Flex::new(d_snes_pins.4),
            Flex::new(d_snes_pins.5),
            Flex::new(d_snes_pins.6),
        ];
        #[cfg(any(feature = "snes", feature = "md"))] let irq_snes = Flex::new(irq_snes_pin);

        #[cfg(feature = "md")]let asout = Output::new(asout_pin, Level::High, Default::default());
        #[cfg(feature = "md")]let clk = Output::new(clk_pin, Level::High, Default::default());
        #[cfg(feature = "md")]let time: Output<'_> = Output::new(time_pin, Level::High, Default::default());

        /*
        let mapper = 0;
        let prglo = 0;
        let prghi = 1;
        let chrlo = 0;
        let chrhi = 1;
        let ramlo = 0;
        let ramhi = 2;

        let mapper = 4;
        let prglo = 1;
        let prghi = 5;
        let chrlo = 0;
        let chrhi = 6;
        let ramlo = 0;
        let ramhi = 1;
        */
        /*
        let mapper = 0;
        let prgsize = 1;
        let chrsize = 1;
        let prg = 32; // KB
        let chr = 8; // KB
        */
        /*
        let mapper = 0;
        let prgsize = 0;
        let chrsize = 1;
        let prg = 16; // KB
        let chr = 8; // KB
        */
        /*
        let mut mapper: u8 = 4;
        let mut prgsize: u8 = 4;
        let mut chrsize: u8 = 5;
        let mut prg: u16 = 256; // KB
        let mut chr: u16 = 128; // KB
        */
        #[cfg(feature = "nes")] let config = DumperConfig {
            mapper: 1,
            prgsize: 3,
            chrsize: 0,
            prg: 128,
            chr: 0,
            #[cfg(feature = "md")] segaSram16bit: false,
        };

       return Self {
            m2,
            pgr_ce,
            #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] chr_wr,
            ciram_ce,
            chr_rd,
            irq,
            prg_rw,
            a,
            #[cfg(any(feature = "snes", feature = "ms",feature = "md"))] ciram_a10,
            d,
            #[cfg(any(feature = "snes", feature = "md"))] a15,
            #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] reset,
            #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] cs,
            #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] wr,
            #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] rd,
            #[cfg(any(feature = "snes"))] refresh,
            #[cfg(any(feature = "snes", feature = "md"))] expand,
            #[cfg(any(feature = "snes", feature = "ms", feature = "md"))] d_snes,
            #[cfg(any(feature = "snes", feature = "md"))] irq_snes,
            #[cfg(feature = "md")] asout,
            #[cfg(feature = "md")] clk,
            #[cfg(feature = "md")] time,
            in_channel,
            out_channel,
            buffer,
            #[cfg(feature = "nes")] config,
        }
    }

    #[cfg(feature = "nes")]
    fn set_address(&mut self, address: u16) {
        for index in 0..self.a.len() - 1 {
            self.a[index].set_level(Level::from((address & (1 << index)) > 0));
        }
        // PPU /A13
        self.a[self.a.len()-1].set_level(Level::from((address & (1 << 13)) == 0));
    }

    #[cfg(feature = "nes")]
    fn set_mode_read(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_as_input(Pull::Up);
        }
    }

    #[cfg(feature = "nes")]
    fn set_write_mode(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_low();
            pin.set_as_output(Default::default());
        }
    }

    #[cfg(feature = "nes")]
    fn set_prg_read(&mut self){
        self.prg_rw.set_high();
    }

    #[cfg(feature = "nes")]
    fn set_prg_write(&mut self){
        self.prg_rw.set_low();
    }

    #[cfg(feature = "nes")]
    fn set_romsel_low(&mut self){
        self.pgr_ce.set_low();
    }

    #[cfg(feature = "nes")]
    fn set_romsel_high(&mut self){
        self.pgr_ce.set_high();
    }

    #[cfg(feature = "nes")]
    fn set_romsel(&mut self, address: u16) {
    if address & 0x8000 > 0 {
        self.set_romsel_low();
    } else {
        self.set_romsel_high();
    }
    }

    #[cfg(feature = "nes")]
    fn set_phy2_high(&mut self){
        self.m2.set_high();
    }

    #[cfg(feature = "nes")]
    fn set_phy2_low(&mut self){
        self.m2.set_low();
    }

    #[cfg(feature = "nes")]
    fn set_chr_read_high(&mut self){
        self.chr_rd.set_high();
    }

    #[cfg(feature = "nes")]
    fn set_chr_read_low(&mut self){
        self.chr_rd.set_low();
    }

    #[cfg(feature = "nes")]
    fn set_romsel_low_and_m2_high(&mut self){
        self.m2.set_high();
        self.pgr_ce.set_low();
    }

    #[cfg(feature = "nes")]
    fn set_romsel_high_and_m2_low(&mut self){
        self.m2.set_low();
        self.pgr_ce.set_high();
    }

    #[cfg(feature = "nes")]
    fn read_data(&mut self) -> u8{
        let mut data = 0;
        for (index, pin) in self.d.iter().enumerate() {
            data |= (pin.is_high() as u8) << index;
        }
        data
    }

    #[cfg(feature = "nes")]
    fn write_data(&mut self, data: u8){
        for (index, pin) in self.d.iter_mut().enumerate() {
            pin.set_level(Level::from((data & (1 << index)) > 0));
        }
    }

    #[cfg(feature = "nes")]
    async fn write_prg_byte(&mut self, address: u16, data: u8) {
        self.set_phy2_low();
        self.set_romsel_high();
        self.set_write_mode();
        self.set_prg_write();
        self.write_data(data);

        self.set_address(address);  // PHI2 low, ROMSEL always HIGH
        // Timer::after_micros(1).await; //  _delay_us(1);
        self.set_phy2_high();
        // Timer::after_micros(10).await; //_delay_us(10);
        self.set_romsel(address);  // ROMSEL is low if need, PHI2 high
        Timer::after_micros(1).await;  // WRITING
        // Timer::after_millis(1).await; //_delay_ms(1); // WRITING
        // PHI2 low, ROMSEL high
        self.set_phy2_low();
        Timer::after_micros(1).await;  // WRITING
        self.set_romsel_high();
        // Back to read mode
        // Timer::after_micros(1).await; //  _delay_us(1);
        self.set_prg_read();
        self.set_mode_read();
        self.set_address(0);
        // Set phi2 to high state to keep cartridge unreseted
        // Timer::after_micros(1).await; //  _delay_us(1);
        self.set_phy2_high();
        // Timer::after_micros(1).await; //  _delay_us(1);
    }

    #[cfg(feature = "nes")]
    async fn read_prg_byte(&mut self, address: u16) -> u8 {
        self.set_mode_read();
        self.set_prg_read();
        self.set_romsel_high();
        self.set_address(address);
        self.set_phy2_high();
        self.set_romsel(address);
        Timer::after_micros(1).await;
        Self::retry_read::<_,BYTE_READ_RETRIES>(|| self.read_data()).await
    }

    #[cfg(feature = "nes")]
    async fn read_chr_byte(&mut self, address: u16) -> u8 {
        self.set_mode_read();
        self.set_phy2_high();
        self.set_romsel_high();
        self.set_address(address);
        self.set_chr_read_low();
        Timer::after_micros(1).await;
        let result = Self::retry_read::<_,BYTE_READ_RETRIES>(|| self.read_data()).await;
        self.set_chr_read_high();
        result
    }

    #[cfg(feature = "nes")]
    async fn write_reg_byte(&mut self, address: u16, data: u8) {  // FIX FOR MMC1 RAM CORRUPTION
        self.set_phy2_low();
        self.set_romsel_high();
        self.set_write_mode();
        self.set_prg_write();
        self.write_data(data);

        self.set_address(address);  // PHI2 low, ROMSEL always HIGH
        // DIRECT PIN TO PREVENT RAM CORRUPTION
        // DIFFERENCE BETWEEN M2 LO AND ROMSEL HI MUST BE AROUND 33ns
        // IF TIME IS GREATER THAN 33ns THEN WRITES TO 0xE000/0xF000 WILL CORRUPT RAM AT 0x6000/0x7000
        //PORTF = 0b01111101;  // ROMSEL LO/M2 HI
        self.set_romsel_low_and_m2_high();
        //PORTF = 0b01111110;  // ROMSEL HI/M2 LO
        self.set_romsel_high_and_m2_low();
        Timer::after_micros(1).await;
        // Back to read mode
        self.set_prg_read();
        self.set_mode_read();
        self.set_address(0);
        // Set phi2 to high state to keep cartridge unreseted
        self.set_phy2_high();
    }

    #[cfg(feature = "nes")]
    async fn write_mmc1_byte(&mut self, address: u16, data: u8) {
        if address >= 0xE000 {
            for i in 0..5u8 {
                self.write_reg_byte(address, data >> i).await;
            }
        } else {
            for j in 0..5u8 {
                self.write_prg_byte(address, data >> j).await;  // shift 1 bit into temp register
            }
        }
    }

    #[cfg(feature = "nes")]
    async fn retry_read<F, const N: usize>(mut f: F) -> u8
    where
        F: FnMut() -> u8,
    {
        let mut values = [0u8; N];

        for i in 0..N {
            values[i] = f();
            Timer::after_micros(1).await;
        }

        let mut best_val = values[0];
        let mut best_count = 1;

        for i in 0..N {
            let mut count = 1;
            for j in (i + 1)..N {
                if values[j] == values[i] {
                    count += 1;
                }
            }
            if count > best_count {
                best_count = count;
                best_val = values[i];
            }
        }

        best_val
    }

    #[cfg(feature = "nes")]
    async fn dump_prg(&mut self, base: u16, address: u16) {
        for x in 0..self.buffer.len() {
             self.buffer[x] = self.read_prg_byte(base + address + x as u16).await;
        }
        self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
    }

    #[cfg(feature = "nes")]
    async fn dump_chr(&mut self, address: u16) {
        for x in 0..self.buffer.len() {
            self.buffer[x] = self.read_chr_byte(address + x as u16).await;
        }
        self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
    }

    #[cfg(feature = "nes")]
    async fn dump_bank_prg(&mut self, from: u16, to: u16, base: u16) {
        for address in (from..to).step_by(Msg::DATA_CHANNEL_SIZE) {
            self.dump_prg(base, address).await;
        }
    }

    #[cfg(feature = "nes")]
    async fn dump_bank_chr(&mut self, from: u16, to: u16) {
        for address in (from..to).step_by(Msg::DATA_CHANNEL_SIZE) {
            self.dump_chr(address).await;
        }
    }

    pub async fn dump(&mut self) {
        let receiver = self.in_channel.receiver();
        loop {
            match receiver.receive().await {
                Msg::Start {console} => {
                    match console {
                        #[cfg(feature = "nes")] MsgStartConsole::Nes => {self.dump_nes().await;}
                        #[cfg(feature = "snes")] MsgStartConsole::Snes => {self.dump_snes().await;}
                        #[cfg(feature = "ms")] MsgStartConsole::Sms => {self.dump_sms().await;}
                        #[cfg(feature = "md")] MsgStartConsole::Md => {self.dump_md().await;}
                    };
                }
                #[cfg(feature = "nes")]
                Msg::DumpSetupDataChanged { field, value } => {
                    let field_encoded = str::from_utf8(&field).unwrap();
                    match field_encoded {
                        "mapper\0\0\0\0\0\0\0\0\0\0" => {
                            self.config.mapper = value[0]
                        }
                        "prgsize\0\0\0\0\0\0\0\0\0" => {
                            self.config.prgsize = value[0]
                        }
                        "chrsize\0\0\0\0\0\0\0\0\0" => {
                            self.config.chrsize = value[0]
                        }
                        "prg\0\0\0\0\0\0\0\0\0\0\0\0\0" => {
                            self.config.prg = u16::from_ne_bytes(value[0..2].try_into().unwrap())
                        }
                        "chr\0\0\0\0\0\0\0\0\0\0\0\0\0" => {
                            self.config.chr = u16::from_ne_bytes(value[0..2].try_into().unwrap())
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(feature = "nes")]
    async fn dump_nes(&mut self) {
        for index in 0..self.a.len() - 1 {
            self.a[index].set_as_output(Default::default());
        }
        self.a[self.a.len()-1].set_as_output(Default::default());
        for dpin in &mut self.d {
            dpin.set_as_input(Pull::Up);
        }
        self.ciram_ce.set_as_input(Pull::Up);
        self.irq.set_as_input(Pull::Up);
        self.out_channel.send(Msg::DumpSetupData{ rom_size:
            ((self.config.prg as u32 + self.config.chr as u32) * 1024) + 16
        }).await;

        // 16 byte header
        self.buffer[..4].copy_from_slice(&[0x4Eu8, 0x45u8, 0x53u8, 0x1Au8]);
        self.buffer[4] = (self.config.prg / 16) as u8;
        self.buffer[5] = (self.config.chr / 8) as u8;
        self.buffer[6] = (self.config.mapper & 0xF) << 4;
        self.buffer[7..16].copy_from_slice(&[0x00u8; 9]);
        self.out_channel.send(Msg::Data { data: *self.buffer, length: 16 }).await;

        self.read_prg(self.config.mapper, self.config.prgsize).await;
        if self.config.chrsize > 0 {
            self.read_chr(self.config.mapper, self.config.chrsize).await;
        }
        self.out_channel.send(Msg::End).await;
    }

    #[cfg(feature = "nes")]
    async fn read_prg(&mut self, mapper: u8, size: u8) {
        self.set_address(0);
        Timer::after_micros(1).await;
        let base: u16 = 0x8000;
        let mut finalize = true;
        match mapper {
            0 => {
                let banks = 1 << size;
                self.dump_bank_prg(0x0, 0x4000 * banks, base).await;
            },
            1 => {
                if size == 1 {
                    self.write_prg_byte(0x8000, 0x80).await;
                    self.dump_bank_prg(0x0000, 0x8000, base).await;
                } else {
                    let banks = 1u8 << size;
                    for i in 0..banks {
                        self.write_prg_byte(0x8000, 0x80).await;
                        self.write_mmc1_byte(0x8000, 0x0C).await;
                        if size > 4 {
                            self.write_mmc1_byte(0xA000, 0x0C).await;
                        }
                        if i > 15 {
                            self.write_mmc1_byte(0xA000, 0x10).await;
                        }
                        self.write_mmc1_byte(0xE000, i).await;
                        self.dump_bank_prg(0x0000, 0x4000, base).await;
                    }
                }
            },
            4 => {
                let banks = (1u16 << size) * 2;
                if banks > 256 {
                    panic!("Address overflow");
                }
                self.write_prg_byte(0xA001, 0x80).await;  // Block Register - PRG RAM Chip Enable, Writable
                for i in 0..banks {
                    self.write_prg_byte(0x8000, 0x06).await;  // PRG Bank 0 ($8000-$9FFF)
                    self.write_prg_byte(0x8001, i as u8).await;
                    self.dump_bank_prg(0x0, 0x2000, base).await;
                }
            },
            _ => {
                finalize = false
            }
        }
        if finalize {
            self.set_address(0);
            self.set_phy2_high();
            self.set_romsel_high();
        }
    }

    #[cfg(feature = "nes")]
    async fn read_chr(&mut self, mapper: u8, size: u8) {
        self.set_address(0);
        Timer::after_micros(1).await;
        match mapper {
            0 => {
                self.dump_bank_chr(0x0, 0x2000).await;
            },
            4 => {
                let banks = (1u16 << size) * 4;
                if banks > 256 {
                    panic!("Address overflow");
                }
                self.write_prg_byte(0xA001, 0x80).await;
                for i in 0..banks {
                    self.write_prg_byte(0x8000, 0x02).await;
                    self.write_prg_byte(0x8001, i as u8).await;
                    self.dump_bank_chr(0x1000, 0x1400).await;
                }
            }
            _ => {}
        }
    }

    #[cfg(feature = "snes")]
    fn set_address_a(&mut self, address: u16) {
        let mut index = 0;
        self.m2.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..8 {
            self.d[d_index].set_level(Level::from((address & (1 << (index + d_index))) > 0));
        }
    }

    #[cfg(feature = "snes")]
    fn set_address_b(&mut self, address: u8) {
        for index in 0..8 {
            self.a[index].set_level(Level::from((address & (1 << (index))) > 0));
        }
    }

    /*
    #[cfg(feature = "snes")]
    fn set_address_p(&mut self, address: u8) {
        for index in 8..15 {
            self.a[index].set_level(Level::from((address & (1 << (index-8))) > 0));
        }
        self.a15.set_level(Level::from((address & (1 << 7)) > 0));
    } */

    #[cfg(feature = "snes")]
    fn set_d_snes_pullup(&mut self) {
        for index in 0..7 {
            self.d_snes[index].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
    }

    #[cfg(feature = "snes")]
    fn read_snes_data(&mut self) -> u8 {
        let mut data = 0;
        for (index, pin) in self.d_snes.iter().enumerate() {
            let true_index = if index < 2 {index} else {index+1} ;
            data |= (pin.is_high() as u8) << true_index;
        }
        data |= (self.ciram_a10.is_high() as u8) << 2;
        data
    }

    #[cfg(feature = "snes")]
    fn set_reset_high(&mut self){
        self.reset.set_high();
    }

    /*
    #[cfg(feature = "snes")]
    fn set_reset_low(&mut self){
        self.reset.set_low();
    } */

    #[cfg(feature = "snes")]
    fn set_wr_high(&mut self){
        self.wr.set_high();
    }

    /*
    #[cfg(feature = "snes")]
    fn set_wr_low(&mut self){
        self.wr.set_low();
    }

    #[cfg(feature = "snes")]
    fn set_rd_high(&mut self){
        self.rd.set_high();
    } */

    #[cfg(feature = "snes")]
    fn set_rd_low(&mut self){
        self.rd.set_low();
    }

    /*
    #[cfg(feature = "snes")]
    fn set_cs_high(&mut self){
        self.cs.set_high();
    } */

    #[cfg(feature = "snes")]
    fn set_cs_low(&mut self){
        self.cs.set_low();
    }

    /*
    #[cfg(feature = "snes")]
    fn set_refresh_high(&mut self){
        self.refresh.set_high();
    } */

    #[cfg(feature = "snes")]
    fn set_refresh_low(&mut self){
        self.refresh.set_low();
    }

    #[cfg(feature = "snes")]
    fn data_in(&mut self) {
        self.set_d_snes_pullup();
    }

    #[cfg(feature = "snes")]
    fn control_in_snes(&mut self) {
        self.set_wr_high();
        self.set_cs_low();
        self.set_rd_low();
    }

    #[cfg(feature = "snes")]
    async fn dump_snes(&mut self) {
        for index in 0..self.a.len() {
            self.a[index].set_as_output(Default::default());
        }
        self.a15.set_as_output(Default::default());

        self.ciram_ce.set_as_output(Default::default());
        self.ciram_ce.set_low();
        self.irq.set_as_output(Default::default());
        self.irq.set_low();
        for d_index in 0..8 {
            self.d[d_index].set_as_output(Default::default());
            self.d[d_index].set_low();
        }
        self.irq_snes.set_as_input(Pull::None);
        self.expand.set_as_input(Pull::None);

        self.set_reset_high();
        self.set_wr_high();
        self.set_cs_low();
        self.set_rd_low();

        self.set_refresh_low();

        let (rom_size, num_banks, rom_type) = self.get_cart_info_snes().await;
        self.out_channel.send(Msg::DumpSetupData{ rom_size: match rom_type {
            v if v == SnesRomType::LO as u8 => {(0x10000 - 0x8000) * num_banks as u32},
            v if v == SnesRomType::HI as u8 => {0x10000 * num_banks as u32},
            _ => {0}
        }}).await;
        self.read_rom_snes(rom_size, num_banks, rom_type).await;
        self.out_channel.send(Msg::End).await;
    }

    #[cfg(feature = "snes")]
    async fn get_cart_info_snes(&mut self) -> (u8, u8, u8) {
        self.set_address_b(0b11000000);
        for curr_byte in 0..1024 {
            self.set_address_a(curr_byte);
            Timer::after_nanos(375).await;
        }
        self.check_cart_snes().await
    }

    #[cfg(feature = "snes")]
    async fn check_cart_snes(&mut self) -> (u8, u8, u8) {
        self.data_in();

        let header_start = 0xFFB0;
        let mut snes_header = [0u8;80];
        self.set_address_b(0x00);
        for c in 0..80 {
            let curr_byte = header_start + c as u16;
            self.set_address_a(curr_byte);
            Timer::after_nanos(75000).await;

            snes_header[c] = self.read_snes_data();
        }
        let mut rom_type = match snes_header[(0xFFD5 - header_start) as usize] {
            v if ((v >> 5) != 1) => {SnesRomType::LO as u8},
            0x35 => {SnesRomType::EX as u8},
            0x3A  => {SnesRomType::HI as u8},
            v => {v & 1},
        };

        let rom_chips = snes_header[(0xFFD6 - header_start) as usize];
        let mut rom_size = 1;
        let mut num_banks = 0;
        if rom_chips == 69 {
            rom_size = 48;
            num_banks = 96;
            rom_type = SnesRomType::HI as u8;
        } else if rom_chips == 67 {
            rom_size = 32;
            num_banks = 64;
            rom_type = SnesRomType::HI as u8;
        } else if rom_chips == 243 {
            let cx4_type = snes_header[(0xFFC9 - header_start) as usize] & 0xF;
            if cx4_type == 2 {  // X2
                rom_size = 12;
                num_banks = 48;
            } else if cx4_type == 3 {  // X3
                rom_size = 16;
                num_banks = 64;
            }
        } else if rom_chips == 245 && rom_type == SnesRomType::HI as u8 {
            rom_size = 24;
            num_banks = 48;
        } else if rom_chips == 249 && rom_type == SnesRomType::HI as u8 {
            rom_size = 40;
            num_banks = 80;
        } else {
            let rom_size_exp = snes_header[(0xFFD7 - header_start) as usize] - 7;
            for _ in 0..rom_size_exp {
                rom_size *= 2;
            }
            if rom_type == SnesRomType::EX as u8 || rom_type == SnesRomType::SA as u8 {
                num_banks = rom_size as u8 * 2
            } else {
                num_banks = ((rom_size as usize * 1024 * (1024 / 8)) / (0x8000 + (rom_type as usize * 0x8000))) as u8;
            }
        }

        (rom_size, num_banks, rom_type)
    }

    #[cfg(feature = "snes")]
    async fn read_rom_snes(&mut self, rom_size: u8,  num_banks: u8, rom_type: u8) {
        self.data_in();
        self.control_in_snes();
        match rom_type {
            v if v == SnesRomType::LO as u8 =>  {
                if rom_size > 24 {
                    // ROM > 96 banks (up to 128 banks)
                    self.read_lo_rom_banks(0x80, num_banks + 0x80).await;
                } else {
                    self.read_lo_rom_banks(0, num_banks).await;
                }
            }
            v if v == SnesRomType::HI as u8 =>  {self.read_hi_rom_banks(192, num_banks + 191).await;}
            _ => {}
        }
    }

    #[cfg(feature = "snes")]
    async fn read_lo_rom_banks(&mut self, start: u8, end: u8) {
        for curr_bank in start..end {
            self.set_address_b(curr_bank);
            let range = 0x8000..=0xFFFF;
            for chunk_start in range.step_by(Msg::DATA_CHANNEL_SIZE) {
                let bytes_range = chunk_start..=(chunk_start - 1 + Msg::DATA_CHANNEL_SIZE as u16).min(0xFFFF);
                let bytes_len = bytes_range.len();
                for (c, curr_byte) in bytes_range.enumerate() {
                    self.set_address_a(curr_byte);
                    Timer::after_nanos(375).await;
                    self.buffer[c] = self.read_snes_data();
                }
                self.out_channel.send(Msg::Data{data: *self.buffer, length: bytes_len}).await;
            }
        }
    }

    #[cfg(feature = "snes")]
    async fn read_hi_rom_banks(&mut self, start: u8, end: u8) {
        for curr_bank in start..=end {
            self.set_address_b(curr_bank);
            let range = 0..=0xFFFF;
            for chunk_start in range.step_by(Msg::DATA_CHANNEL_SIZE) {
                let bytes_range = chunk_start..=((chunk_start as u32 + Msg::DATA_CHANNEL_SIZE as u32) - 1 ).min(0xFFFF) as u16;
                let bytes_len = bytes_range.len();
                for (c, curr_byte) in bytes_range.enumerate() {
                    self.set_address_a(curr_byte);
                    Timer::after_nanos(375).await;
                    self.buffer[c] = self.read_snes_data();
                }
                self.out_channel.send(Msg::Data{data: *self.buffer, length: bytes_len}).await;
            }
        }
    }

    #[cfg(feature = "ms")]
    async fn dump_sms(&mut self) {
        let cart_size = self.setup_sms().await;
        self.out_channel.send(Msg::DumpSetupData{ rom_size: cart_size }).await;
        self.read_rom_sms(cart_size).await;
        self.out_channel.send(Msg::End).await;
    }

    #[cfg(feature = "ms")]
    fn set_address_sms(&mut self, address: u16) {
        let mut index = 0;
        self.m2.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..7 {
            self.d[d_index].set_level(Level::from((address & (1 << (index + d_index))) > 0));
        }
    }

    #[cfg(feature = "ms")]
    fn set_data_sms(&mut self, data: u8) {
        for d_snes_index in 0..=1 {
            self.d_snes[d_snes_index].set_level(Level::from((data & (1 << (d_snes_index))) > 0));
        }
        self.ciram_a10.set_level(Level::from((data & (1 << 2)) > 0));
        for d_snes_index in 2..=6 {
            self.d_snes[d_snes_index].set_level(Level::from((data & (1 << (d_snes_index + 1))) > 0));
        }
    }

    #[cfg(feature = "ms")]
    async fn write_byte_sms(&mut self, my_address: u16, my_data: u8) {
        for i in 0..7 {
            self.d_snes[i].set_as_output(Default::default());
        }
        self.ciram_a10.set_as_output(Default::default());
        self.set_address_sms(my_address);
        self.cs.set_level(Level::from((my_address & (1 << 15)) > 0));
        self.set_data_sms(my_data);
        Timer::after_nanos(63).await;
        self.a[1].set_low();
        self.wr.set_low();
        Timer::after_nanos(63).await;
        self.wr.set_high();
        self.a[1].set_high();
        Timer::after_nanos(63).await;
        for i in 0..7 {
            self.d_snes[i].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
    }

    #[cfg(feature = "ms")]
    fn read_nibble(&self, data: u8, number: u8) -> u8 {
        (data >> (number * 4)) & 0xF
    }

    #[cfg(feature = "ms")]
    fn get_data_sms(&mut self) -> u8 {
        let mut data = 0;
        for (index, pin) in self.d_snes.iter().enumerate() {
            let true_index = if index < 2 {index} else {index+1} ;
            data |= (pin.is_high() as u8) << true_index;
        }
        data |= (self.ciram_a10.is_high() as u8) << 2;
        data
    }

    #[cfg(feature = "ms")]
    async fn read_byte_sms(&mut self, my_address: u16) -> u8 {
        for d_snes_index in 0..7 {
            self.d_snes[d_snes_index].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
        self.set_address_sms(my_address);
        self.cs.set_level(Level::from((my_address & (1 << 15)) > 0));
        Timer::after_nanos(63).await;
        self.a[1].set_low();
        self.rd.set_low();
        Timer::after_nanos(63).await;
        let temp_byte = self.get_data_sms();
        self.rd.set_high();
        self.a[1].set_high();
        Timer::after_nanos(63).await;
        temp_byte
    }

    #[cfg(feature = "ms")]
    async fn get_cart_info_sms(&mut self) -> u32 {
        let card_nib_byte = self.read_byte_sms(0x7FFF).await;
        let cart_nib = self.read_nibble(card_nib_byte, 0);
        let mut cart_size = match cart_nib {
            0x0a => 8192,
            0x0b => 16384,
            0x0c => 32768,
            0x0d => 49152,
            0x0e => 65536,
            0x0f => 131072,
            0x00 => 262144,
            0x01 => 524288,
            0x02 => 524288,
            0x03 => 131072,
            _ => 49152,
        };
        let mut rom_name = [0u8;8];
        for char_index in 0..rom_name.len() {
            rom_name[char_index] = self.read_byte_sms(0x7FF0 + char_index as u16).await;
        }
        if &rom_name == b"TMR SEGA" {
            let mut bank = 1u8;
            let mut rom_name_buf = [0u8;8];
            while bank < 64 {
                bank += 1;
                self.write_byte_sms(0xFFFE, bank).await;
                for char_index in 0..rom_name_buf.len() {
                    rom_name_buf[char_index] = self.read_byte_sms(0x7FF0 + char_index as u16).await;
                }
                if rom_name == rom_name_buf {
                    break;
                }
            }
            if bank > 2 {
                cart_size = (bank - 1) as u32 * 16384;
            }
            self.write_byte_sms(0xFFFE, 1).await;
        }
        cart_size
    }

    #[cfg(feature = "ms")]
    async fn setup_sms(&mut self) -> u32 {
        self.ciram_ce.set_as_output(Default::default());
        self.irq.set_as_output(Default::default());
        for i in 0..7 {
            self.d[i].set_as_output(Default::default());
        }
        self.a[1].set_as_output(Default::default());
        self.a[15].set_as_output(Default::default());
        self.reset.set_high();
        self.wr.set_high();
        self.rd.set_high();
        self.a[1].set_high();
        self.write_byte_sms(0xFFFC, 0).await;
        self.write_byte_sms(0xFFFD, 0).await;
        self.write_byte_sms(0xFFFE, 1).await;
        self.write_byte_sms(0xFFFF, 2).await;
        Timer::after_millis(400).await;
        self.get_cart_info_sms().await
    }

    #[cfg(feature = "ms")]
    async fn read_rom_sms(&mut self, cart_size: u32) {
        let mut bank_size = 16384;
        if cart_size == 32768 {
            bank_size = cart_size as u16;
        }
        let banks_count = cart_size / (bank_size as u32);
        for curr_bank in 0x0..banks_count as u8 {
            self.write_byte_sms(0xFFFF, curr_bank).await;
            Timer::after_nanos(63).await;
            for curr_buffer in (0..bank_size).step_by(self.buffer.len()) {
                for curr_byte in 0..self.buffer.len() as u16 {
                    self.buffer[curr_byte as usize] = self.read_byte_sms((if cart_size == 32768 { 0 } else { 0x8000 }) + curr_buffer + curr_byte).await;
                }
                self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
            }
            Timer::after_nanos(63).await;
        }
    }

    #[cfg(feature = "md")]
    async fn dump_md(&mut self) {
        let (cart_size, realtec , is_svp, snk_mode, cart_size_lockon)= self.setup_md().await;
        self.out_channel.send(Msg::DumpSetupData{ rom_size: cart_size }).await;
        match realtec {
            true => self.read_realtec_md(cart_size).await,
            false => self.read_rom_md(cart_size, is_svp, snk_mode, cart_size_lockon).await,
        }
        self.out_channel.send(Msg::End).await;
    }

    #[cfg(feature = "md")]
    fn data_in_md(&mut self) {
        for d_snes_index in 0..7 {
            self.d_snes[d_snes_index].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
        for index in 8..15 {
            self.a[index].set_as_input(Pull::Up);
        }
        self.a15.set_as_input(Pull::Up);
    }

    #[cfg(feature = "md")]
    fn pulse_clock(&mut self, n: u8) {
        let start_level = self.clk.is_set_high();
        for i in 0..n {
            self.clk.set_level(Level::from((i%2 > 0) == start_level));
        }
    }

    #[cfg(feature = "md")]
    async fn read_word_md(&mut self, address: u32) -> u16 {
        let mut index = 0;
        self.m2.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..self.d.len() {
            self.d[d_index].set_level(Level::from((address & (1 << (index + d_index))) > 0));
        }
        index += self.d.len();
        for a_index in 0..8 {
            self.a[a_index].set_level(Level::from((address & (1 << (index + a_index))) > 0));
        }
        Timer::after_nanos(63).await;

        self.cs.set_low();
        self.rd.set_low();
        self.asout.set_low();
        self.expand.set_low();
        self.pulse_clock(10);

        Timer::after_nanos(375).await;

        let mut temp_word = 0;
        for (index, pin) in self.a[8..15].iter().enumerate() {
            temp_word |= (pin.is_high() as u16) << (index + 8);
        }
        temp_word |= (self.a15.is_high() as u16) << 15;
        for (index, pin) in self.d_snes.iter().enumerate() {
            let true_index = if index < 2 {index} else {index+1} ;
            temp_word |= (pin.is_high() as u16) << true_index;
        }
        temp_word |= (self.ciram_a10.is_high() as u16) << 2;

        self.cs.set_high();
        self.rd.set_high();
        self.asout.set_high();
        self.expand.set_high();
        self.pulse_clock(10);

        Timer::after_nanos(375).await;

        return temp_word;
    }

    #[cfg(feature = "md")]
    fn copy_to_rom_name_md(&self, output: &mut [u8], input: &[u8], length: usize) -> usize {
        let mut my_length = 0;
        for i in 0..48 {
            if ((input[i] >= b'0' && input[i] <= b'9') || (input[i] >= b'A' && input[i] <= b'z')) && my_length < length {
                my_length+=1;
                output[my_length] = input[i];
            }
        }
        return my_length
    }

    #[cfg(feature = "md")]
    fn data_out_md(&mut self) {
        for d_snes_index in 0..7 {
            self.d_snes[d_snes_index].set_as_output(Default::default());
        }
        self.ciram_a10.set_as_output(Default::default());
        for index in 8..15 {
            self.a[index].set_as_output(Default::default());
        }
        self.a15.set_as_output(Default::default());
    }

    #[cfg(feature = "md")]
    async fn write_ssf2_map(&mut self, my_address: u32, my_data: u16) {
        self.data_out_md();

        self.time.set_high();

        let mut index = 0;
        self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..self.d.len() {
            self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
        }
        index += self.d.len();
        for a_index in 0..8 {
            self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
        }

        index = 0;
        for d_snes_index in 0..=1 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index))) > 0));
        }
        self.ciram_a10.set_level(Level::from((my_data & (1 << index)) > 0));
        for d_snes_index in 2..=6 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index + 1))) > 0));
        }
        index += 8;
        for a_index in 8..=14 {
            self.a[a_index].set_level(Level::from((my_data & (1 << (a_index + index))) > 0));
        }
        index += 7;
        self.a15.set_level(Level::from((my_data & (1 << index)) > 0));

        Timer::after_nanos(125).await;

        self.time.set_low();
        self.wr.set_low();

        Timer::after_nanos(750).await;

        self.wr.set_high();
        self.time.set_high();

        self.data_in_md();
    }

    #[cfg(feature = "md")]
    async fn get_cart_info_md(&mut self) -> (u32, bool, bool, u8, u32) {
        let cart_size_lockon = 0;

        self.data_in_md();
        let mut cart_size = (((self.read_word_md(0xD2).await as u32) << 16) |
                               (self.read_word_md(0xD3).await as u32)) + 1;


        //let is_32x = self.read_word_md(0x104 / 2).await == 0x2033u16 &&
        //                   self.read_word_md(0x106 / 2).await == 0x3258u16;

        let mut chksum = self.read_word_md(0xC7).await;

        let mut id = [0u8;14];
        for c in (0usize..id.len()).step_by(2) {
            let my_word = self.read_word_md((0x180+(c as u32))/2).await;
            let lo_byte = (my_word & 0xFF) as u8;
            let hi_byte = (my_word >> 8) as u8;
            id[c] = hi_byte;
            id[c + 1] = lo_byte;
        }

        let mut sd_buffer = [0u8;48];
        for c in (0usize..sd_buffer.len()).step_by(2) {
            let my_word = self.read_word_md((0x150+(c as u32))/2).await;
            let lo_byte = (my_word & 0xFF) as u8;
            let hi_byte = (my_word >> 8) as u8;
            sd_buffer[c] = hi_byte;
            sd_buffer[c + 1] = lo_byte;
        }
        let mut rom_name = [0u8;22];
        let rom_name_size = rom_name.len();
        let last_char = self.copy_to_rom_name_md(&mut rom_name, &sd_buffer, rom_name_size - 1);

        let is_svp = &id[..11] == b"GM MK-1229 " || &id[..11] == b"GM G-7001  ";

        (cart_size, chksum) = match cart_size {
            0x400000 => match chksum {
                0xCE25 | 0xE41D | 0xE017 => (0x500000, chksum),
                0x0000 => (0xEAF2F4, chksum),
                0xBCBF | 0x6E1E => (0xEA0000, chksum),
                _ => (cart_size, chksum),
            },
            0x300000 => match chksum {
                0xBC5F | 0x3CDD | 0x44AD | 0x2D9A | 0x5648 | 0x0A29 | 0x7651 | 0x74CA => (0x400000, chksum),
                _ => (cart_size, chksum),
            },
            0x200000 => match chksum {
                0x2078 => (cart_size, 0x9877),
                0xAE95 => (cart_size, 0x56A0),
                _ => (cart_size, chksum),
            },
            0x180000 => match chksum {
                0xFFE2 | 0xF418 | 0xF71D | 0xA884 | 0x7D68 | 0x030D | 0xE975 => (0x200000, chksum),
                _ => (cart_size, chksum),
            },
            0x100000 => match chksum {
                0xCDF5 => (0x400000, 0x603A),
                0xF85F => (0x200000, 0x6965),
                0x4581 => (0x400000, 0x0694),
                _ => (cart_size, chksum),
            },
            0xC0000 => match chksum {
                0x9D79 => (0x100000, chksum),
                _ => (cart_size, chksum),
            },
            0x80000 => match chksum {
                0x06C1 => (0x200000, 0x8473),
                0x5B3A => (0x200000, 0x5613),
                0xD07D => (0x100000, 0xF204),
                0x95C9 | 0x9144 | 0xB8D4 => (0x100000, chksum),
                0xC422 => (cart_size, 0xC751),
                0x0C6A => (cart_size, 0xE1AA),
                0xA760 => (cart_size, 0x97CD),
                0x1404 => (cart_size, 0x53B9),
                _ => (cart_size, chksum),
            },
            0x40000 => match chksum {
                0x8BC6 | 0xB344 => (0x100000, chksum),
                _ => (cart_size, chksum),
            },
            0x20000 => match chksum {
                0x7E50 => (0x100000, 0xD074),
                0x168B => (0x100000, 0xCEE0),
                _ => (cart_size, chksum),
            },
            _ => (cart_size, chksum),
        };

        if &id[..11] == b"GM T-44013 " && chksum == 0xFFFF {
            chksum = 0xC560;
            cart_size = 0xA0000;
        }

        if &rom_name[..12] == b"GMT5604600jJ" && chksum == 0xFFFF {
            let rom_name_string = b"SLAUGHTERSPORT";
            rom_name[..rom_name_string.len()].copy_from_slice(rom_name_string);
            chksum = 0x6BAE;
        }

        if &id[..6] == b"SF-001" && chksum == 0x3E08 {
            cart_size = 0x400000;
        }
        if &id[..6] == b"SF-002" && chksum == 0x12B0 {
            chksum = 0x45C6;
        }
        if b"GM 10101010" == &id[..11] && chksum == 0xC439 {
            chksum = 0x21B0;
            cart_size = 0x100000;
        }
        if b"MU REMUTE01" == &id[..11] && chksum == 0x0000 {
            chksum = 0xB55C;
            cart_size = 0x400000;
        }
        if b"GM REMUTE02" == &id[..11] && chksum == 0x0000 {
            chksum = 0x5426;
            cart_size = 0x400000;
        }
        if b"GM HHARVYSG" == &id[..11] && chksum == 0x0000 {
            chksum = 0xD9D2;
            cart_size = 0x100000;
        }
        if b"GM T-107036" == &id[..11] && chksum == 0x0000 {
            chksum = 0xAA28;
        }
        if b"GM 00000000-43" == &id[..14] && chksum == 0x0000 {
            chksum = 0x921B;
            cart_size = 0x400000;
        }
        if b"GM 00000000-00" == &id[..14] && chksum == 0x1E0C {
            chksum = 0xE7E5;
            cart_size = 0x400000;
        }
        if b"GM 00000000-00" == &id[..14] && chksum == 0x6BD5 {
            chksum = 0x1FEA;
            cart_size = 0x400000;
        }
        if b"GM 00000005-00" == &id[..14] && chksum == 0x9F34 {
            chksum = 0xA094;
            cart_size = 0x400000;
        }
        if b"GM 00000005-00" == &id[..14] && chksum == 0x0E9B {
            chksum = 0x6B4B;
            cart_size = 0x400000;
        }
        if b"GM T-574323-00" == &id[..14] && chksum == 0xAEDD {
            cart_size = 0x400000;
        }
        if b"GM MK-0000 -00" == &id[..14] && chksum == 0xC536 {
            chksum = 0xFAB1;
            cart_size = 0x200000;
        }
        if b"GM CSET0001-02" == &id[..14] && chksum == 0x0000 {
            chksum = 0xE3A9;
        }
        if b"1774          " == &id[..14] && chksum == 0x0000 {
            chksum = 0x6E34;
            cart_size = 0x400000;
        }
        if b"JN-20160131-03" == &id[..14] && chksum == 0x0000 {
            chksum = 0x8040;
            cart_size = 0x400000;
        }
        if b"ROMEOWJULICAT" == &rom_name[..13] && chksum == 0x0000 {
            chksum = 0xB094;
            cart_size = 0x200000;
        }

        let mut snk_mode = 0u8;

        if b"GM MK-1563 -00" == &id && chksum == 0xDFB3 {
            let mut label_lockon = [0u8; 16];
            for c in (0usize..label_lockon.len()).step_by(2) {
                let my_word = self.read_word_md((0x200100+(c as u32))/2).await;
                let lo_byte = (my_word & 0xFF) as u8;
                let hi_byte = (my_word >> 8) as u8;
                label_lockon[c] = hi_byte;
                label_lockon[c + 1] = lo_byte;
            }

            if "SEGA MEGA DRIVE ".as_bytes() == &label_lockon || "SEGA GENESIS    ".as_bytes() == &label_lockon {
                let mut id_lockon = [0u8; 14];
                let chksum_lockon = self.read_word_md(0x1000C7).await;
                let cart_size_lockon = ((self.read_word_md(0x1000D2).await as u32) << 16) | self.read_word_md(0x1000D3).await as u32 + 1;
                for c in (0usize..id_lockon.len()).step_by(2) {
                    let my_word = self.read_word_md((0x200180+(c as u32))/2).await;
                    let lo_byte = (my_word & 0xFF) as u8;
                    let hi_byte = (my_word >> 8) as u8;
                    id_lockon[c] = hi_byte;
                    id_lockon[c + 1] = lo_byte;
                }

                if "GM 00001009-0".as_bytes() == &id_lockon[..13] || "GM 00004049-0".as_bytes() == &id_lockon[..13] {
                    snk_mode = 2;
                } else if "GM 00001051-00".as_bytes() == &id_lockon || "GM 00001051-01".as_bytes() == &id_lockon || "GM 00001051-02".as_bytes() == &id_lockon {
                    snk_mode = 3;
                    self.write_ssf2_map(0x509878, 1).await;
                } else if "GM MK-1079 -00".as_bytes() == &id_lockon {
                    snk_mode = 4;
                } else { // Other game
                    snk_mode = 5;
                }
            }
        }

        let (eep_type, eep_size, mut save_type) = match chksum {
            0x5B9F | 0x694F | 0xBFA9 => Ok(0x101),
            0x16B2 | 0xCC3F | 0x8AE1 | 0xDB97 | 0x7651 | 0xDFE4 => Ok(0x102),
            0x3DE6 => Ok(0x802),
            0xCB78 | 0x6DD9 => Ok(0x2002),
            0xAD23 | 0xEA80 | 0x760F | 0x95E7 => Ok(0x83),
            0x0000 => {
                if self.read_word_md(0xD9).await != 0xE840 {
                    Ok(0x83)
                } else {
                    Err("not found")
                }
            },
            0x7270 | 0xBACC | 0xB939 | 0x487C | 0x740D | 0x0278 | 0x9D79 => Ok(0x83),
            0x8512 | 0xA107 | 0x246A | 0x5807 | 0x2799 | 0xFA57 | 0x8B9F => Ok(0x84),
            0x7E65 => Ok(0x405),
            0x9A5C | 0xC4EE => Ok(0x2005),
            0x7E50 => Ok(0x805),
            0x165E => {
                if self.read_word_md(0x00).await == 0x444E {
                    Ok(0x805)
                } else {
                    Err("not found")
                }
            },
            0x168B => {
                if self.read_word_md(0x00).await == 0x444E {
                    Ok(0x405)
                } else {
                    Err("not found")
                }
            },
            0x12C1 => Ok(0x2005),
            _ => Err("not found"),
        }.map_or((0, 0, 0), |eep_data| (eep_data & 0x7, eep_data & 0xFFF8, 4));

        let bram_check = self.read_word_md(0x00).await;
        let mut bram_size = 0;
        if (bram_check & 0xFF == 0x04 && chksum & 0xFF == 0x04) ||
           (bram_check & 0xFF == 0x06 && chksum & 0xFF == 0x06) {
            let p = 1 << (bram_check & 0xFF);
            bram_size = p * 0x2000u32;
        }


        let mut sram_size = 0;
        if save_type != 4 {
            save_type = 0;
            sram_size = 0;
            let mut sram_base= 0;
            let mut sram_end = 0;
            if self.read_word_md(0xD8).await == 0x5241 {
                let sram_type = self.read_word_md(0xD9).await;
                if sram_type == 0xF820 || sram_type == 0xF840 {
                    sram_base = ((self.read_word_md(0xDA).await as u32) << 16) | (self.read_word_md(0xDB).await as u32);
                    sram_end = ((self.read_word_md(0xDC).await as u32) << 16) | (self.read_word_md(0xDD).await as u32);
                    if sram_base == 0x20000020 && sram_end == 0x00010020 {  // Fix for Psy-o-blade
                        sram_base = 0x200001;
                        sram_end = 0x203fff;
                    }
                    if sram_base == 0x200001 || sram_base == 0x300001 || sram_base == 0x3C0001 {
                        save_type = 1;
                        sram_size = (sram_end - sram_base + 2) / 2;
                        sram_base = sram_base >> 1;
                    } else if sram_base == 0x200000 {
                        save_type = 2;
                        sram_size = (sram_end - sram_base + 1) / 2;
                        sram_base = sram_base / 2;
                    }
                } else if sram_type == 0xE020 {
                    sram_base = ((self.read_word_md(0xDA).await as u32) << 16) | (self.read_word_md(0xDB).await as u32);
                    sram_end = ((self.read_word_md(0xDC).await as u32) << 16) | (self.read_word_md(0xDD).await as u32);

                    if sram_base == 0x200001 {
                        save_type = 3;
                        sram_size = sram_end - sram_base + 2;
                        sram_base = sram_base >> 1;
                    } else if sram_base == 0x200000 {
                        save_type = 3;
                        sram_size = sram_end - sram_base + 1;
                        sram_base = sram_base >> 1;
                    } else if sram_base == 0x3FFC00 {
                        save_type = 0;
                    }
                }
            } else {
                match chksum {
                    0xC2DB => {
                        save_type = 1;
                        sram_base = 0x200001;
                        sram_end = 0x200FFF;
                    },
                    0xD7B6 | 0xFE3E | 0xFDAD | 0x632E | 0xD2BA | 0x44FE => {
                        save_type = 1;
                        sram_base = 0x200001;
                        sram_end = 0x203FFF;
                    },
                    0xDB5E | 0x3428 | 0x43EE => {
                        save_type = 3;
                        sram_base = 0x200001;
                        sram_end = 0x207FFF;
                    },
                    0xBF72 | 0x72EF | 0xD723 | 0x06C1 | 0xDB17 | 0x5B3A | 0x2CF2 | 0xE9B1 | 0xEEE8 => {
                        save_type = 1;
                        sram_base = 0x200001;
                        sram_end = 0x20FFFF;
                    },
                    _ => {}
                }
                if save_type == 1 {
                    sram_size = (sram_end - sram_base + 2) / 2;
                    sram_base = sram_base >> 1;
                } else if save_type == 3 {
                    sram_size = sram_end - sram_base + 2;
                    sram_base = sram_base >> 1;
                }
            }
        }
        if snk_mode >= 2 {
            let mut rom_name_lockon = [0u8; 12];
            let mut sd_buffer = [0u8; 48];
            for c in (0usize..sd_buffer.len()).step_by(2) {
                let my_word = self.read_word_md((0x200150+(c as u32))/2).await;
                let lo_byte = (my_word & 0xFF) as u8;
                let hi_byte = (my_word >> 8) as u8;
                sd_buffer[c] = hi_byte;
                sd_buffer[c + 1] = lo_byte;
            }
            let rom_name_size = rom_name_lockon.len();
            let last_char = self.copy_to_rom_name_md(&mut rom_name_lockon, &sd_buffer, rom_name_size - 1);

            let suffix = match snk_mode {
                2 => "SONIC1".as_bytes(),
                3 => "SONIC2".as_bytes(),
                4 => "SONIC3".as_bytes(),
                5 => &rom_name_lockon,
                _ => "??????".as_bytes(),
            };
            assert!(last_char + suffix.len() <= rom_name.len());
            rom_name[last_char..last_char + suffix.len()].copy_from_slice(suffix);
        }
        let realtec_check1 = self.read_word_md(0x3F080).await;
        let realtec_check2 = self.read_word_md(0x3F081).await;
        let mut realtec = false;
        if realtec_check1 == 0x5345 && realtec_check2 == 0x4741 {
            realtec = true;
            rom_name.copy_from_slice("Realtec".as_bytes());
            cart_size = 0x80000;
        }

        if cart_size < 0x8000 || cart_size > 0xEAF400 {
            for temp_cart_size in (0x20000 / 2 .. 0x400000 / 2).step_by(0x20000 / 2) {
                if self.read_word_md(0x0).await == self.read_word_md(temp_cart_size as u32).await &&
                    self.read_word_md(0x1).await == self.read_word_md(0x1 + temp_cart_size).await &&
                    self.read_word_md(0x2).await == self.read_word_md(0x2 + temp_cart_size).await &&
                    self.read_word_md(0x3).await == self.read_word_md(0x3 + temp_cart_size).await &&
                    self.read_word_md(0x4).await == self.read_word_md(0x4 + temp_cart_size).await &&
                    self.read_word_md(0x5).await == self.read_word_md(0x5 + temp_cart_size).await &&
                    self.read_word_md(0x6).await == self.read_word_md(0x6 + temp_cart_size).await &&
                    self.read_word_md(0x7).await == self.read_word_md(0x7 + temp_cart_size).await &&
                    self.read_word_md(0x8).await == self.read_word_md(0x8 + temp_cart_size).await {
                    cart_size = temp_cart_size;
                    break;
                }
            }
            cart_size *= 2
        }

        (cart_size, realtec, is_svp, snk_mode, cart_size_lockon)
    }

    #[cfg(feature = "md")]
    async fn setup_md(&mut self) -> (u32, bool, bool, u8, u32) {
        self.ciram_ce.set_as_output(Default::default());
        self.a[15].set_as_output(Default::default());
        self.irq.set_as_output(Default::default());
        for pin in self.d.iter_mut() {
            pin.set_as_output(Default::default());
        }
        for a_index in 0..8 {
            self.a[a_index].set_as_output(Default::default());
        }

        self.irq_snes.set_as_output(Default::default());

        self.expand.set_as_output(Default::default());

        for d_snes_index in 0..7 {
            self.d_snes[d_snes_index].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
        for index in 8..15 {
            self.a[index].set_as_input(Pull::Up);
        }
        self.a15.set_as_input(Pull::Up);

        self.reset.set_high();
        self.cs.set_high();
        self.irq_snes.set_high();
        self.wr.set_high();
        self.rd.set_high();

        self.time.set_high();
        self.asout.set_high();

        self.expand.set_high();

        Timer::after_millis(200).await;

        self.get_cart_info_md().await
    }

    #[cfg(feature = "md")]
    async fn write_word_md(&mut self, my_address: u32, my_data: u16) {
        let mut index = 0;
        self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..self.d.len() {
            self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
        }
        index += self.d.len();
        for a_index in 0..8 {
            self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
        }

        index = 0;
        for d_snes_index in 0..=1 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index))) > 0));
        }
        self.ciram_a10.set_level(Level::from((my_data & (1 << index)) > 0));
        for d_snes_index in 2..=6 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index + 1))) > 0));
        }
        index += 8;
        for a_index in 8..=14 {
            self.a[a_index].set_level(Level::from((my_data & (1 << (a_index + index))) > 0));
        }
        index += 7;
        self.a15.set_level(Level::from((my_data & (1 << index)) > 0));

        Timer::after_nanos(125).await;

        self.wr.set_low();
        self.cs.set_low();

        Timer::after_nanos(750).await;

        self.cs.set_high();
        self.wr.set_high();

        Timer::after_nanos(375).await;
    }

    #[cfg(feature = "md")]
    fn write_realtec(&mut self, my_address: u32, my_data: u8) {
        self.data_out_md();

        let mut index = 0;
        self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
        index += 1;
        for d_index in 0..self.d.len() {
            self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
        }
        index += self.d.len();
        for a_index in 0..8 {
            self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
        }

        for a_index in 8..=14 {
            self.a[a_index].set_low();
        }
        self.a15.set_low();
        self.reset.set_high();

        self.cs.set_high();
        index = 0;
        for d_snes_index in 0..=1 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index))) > 0));
        }
        self.ciram_a10.set_level(Level::from(my_data & (1 << index)));
        for d_snes_index in 2..=6 {
            self.d_snes[d_snes_index].set_level(Level::from((my_data & (1 << (d_snes_index + 1))) > 0));
        }

        self.irq_snes.set_low();
        self.wr.set_low();
        self.irq_snes.set_high();
        self.wr.set_high();
        self.data_in_md();
    }

    #[cfg(feature = "md")]
    async fn read_realtec_md(&mut self, cart_size: u32) {
        self.data_in_md();
        self.write_word_md(0x201000, 4).await;
        self.write_realtec(0x200000, 1);
        self.write_realtec(0x202000, 0);

        let mut d = 0;
        for curr_buffer in (0..cart_size/2).step_by(self.buffer.len()) {
            for curr_word in 0..self.buffer.len()/2 {
                let my_word = self.read_word_md(curr_buffer + curr_word as u32).await;
                self.buffer[d] = ((my_word >> 8) & 0xFF) as u8;
                self.buffer[d + 1] = (my_word & 0xFF) as u8;
                d += 2;
            }
        }
        self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
    }

    #[cfg(feature = "md")]
    async fn enable_sram_md(&mut self, enable_sram: bool) {
        self.data_out_md();

        self.d_snes[0].set_level(Level::from(enable_sram));
        self.ciram_a10.set_low();
        for d_snes_index in 1..=6 {
            self.d_snes[d_snes_index].set_low();
        }

        self.time.set_low();

        Timer::after_nanos(375).await;

        self.time.set_high();

        Timer::after_nanos(375).await;

        self.data_in_md();
    }

    #[cfg(feature = "md")]
    async fn read_rom_md(&mut self, cart_size: u32, is_svp: bool, snk_mode: u8, cart_size_lockon: u32) {
        self.data_in_md();
        if 0x200000 < cart_size && cart_size < 0x400000 {
            self.enable_sram_md(false).await;
        }
        if cart_size > 0x400000 {
            self.write_ssf2_map(0x50987E, 6).await;
            self.write_ssf2_map(0x50987F, 7).await;
        }

        let mut offset_ssf2_bank = 0;
        let mut d;

        for curr_buffer in (0..cart_size / 2).step_by(self.buffer.len() / 2) {

            if curr_buffer == 0x200000 {
                self.write_ssf2_map(0x50987E, 8).await;
                self.write_ssf2_map(0x50987F, 9).await;
                offset_ssf2_bank = 1;
            } else if curr_buffer == 0x280000 {
                self.write_ssf2_map(0x50987E, 10).await;
                self.write_ssf2_map(0x50987F, 11).await;
                offset_ssf2_bank = 2;
            } else if curr_buffer == 0x300000 {
                self.write_ssf2_map(0x50987E, 12).await;
                self.write_ssf2_map(0x50987F, 13).await;
                offset_ssf2_bank = 3;
            } else if curr_buffer == 0x380000 {
                self.write_ssf2_map(0x50987E, 14).await;
                self.write_ssf2_map(0x50987F, 15).await;
                offset_ssf2_bank = 4;
            } else if curr_buffer == 0x400000 {
                self.write_ssf2_map(0x50987E, 16).await;
                self.write_ssf2_map(0x50987F, 17).await;
                offset_ssf2_bank = 5;
            } else if curr_buffer == 0x480000 {
                self.write_ssf2_map(0x50987E, 18).await;
                self.write_ssf2_map(0x50987F, 19).await;
                offset_ssf2_bank = 6;
            } else if curr_buffer == 0x500000 {
                self.write_ssf2_map(0x50987E, 20).await;
                self.write_ssf2_map(0x50987F, 21).await;
                offset_ssf2_bank = 7;
            } else if curr_buffer == 0x580000 {
                self.write_ssf2_map(0x50987E, 22).await;
                self.write_ssf2_map(0x50987F, 23).await;
                offset_ssf2_bank = 8;
            } else if curr_buffer == 0x600000 {
                self.write_ssf2_map(0x50987E, 24).await;
                self.write_ssf2_map(0x50987F, 25).await;
                offset_ssf2_bank = 9;
            } else if curr_buffer == 0x680000 {
                self.write_ssf2_map(0x50987E, 26).await;
                self.write_ssf2_map(0x50987F, 27).await;
                offset_ssf2_bank = 10;
            } else if curr_buffer == 0x700000 {
                self.write_ssf2_map(0x50987E, 28).await;
                self.write_ssf2_map(0x50987F, 29).await;
                offset_ssf2_bank = 11;
            }

            d = 0;

            for curr_word in 0..self.buffer.len() / 2 {
                let my_address = curr_buffer + (curr_word as u32) - (offset_ssf2_bank * 0x80000);

                let mut index = 0;
                self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
                index += 1;
                for d_index in 0..self.d.len() {
                    self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
                }
                index += self.d.len();
                for a_index in 0..8 {
                    self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
                }

                Timer::after_nanos(63).await;

                self.cs.set_low();
                self.rd.set_low();
                self.asout.set_low();
                self.expand.set_low();

                if is_svp {
                    self.pulse_clock(10);
                }

                Timer::after_nanos(375).await;

                self.buffer[d] = 0;
                self.buffer[d + 1] = 0;
                for (index, pin) in self.d_snes.iter().enumerate() {
                    let true_index = if index < 2 {index} else {index+1} ;
                    self.buffer[d] |= (pin.is_high() as u8) << true_index;
                }
                self.buffer[d] |= (self.ciram_a10.is_high() as u8) << 2;
                for (index, pin) in self.a[8..15].iter().enumerate() {
                    self.buffer[d + 1] |= (pin.is_high() as u8) << index;
                }
                self.buffer[d + 1] |= (self.a15.is_high() as u8) << 7;

                self.cs.set_high();
                self.rd.set_high();
                self.asout.set_high();
                self.expand.set_high();

                if is_svp {
                    self.pulse_clock(10);
                }
                d += 2;
            }
            self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
        }
        if snk_mode >= 2 {
            for curr_buffer in (0..cart_size_lockon / 2).step_by(self.buffer.len() / 2) {
                d = 0;
                for curr_word in 0..self.buffer.len()/2 {
                    let my_address = curr_buffer + curr_word as u32 + cart_size / 2;
                    let mut index = 0;
                    self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    for d_index in 0..self.d.len() {
                        self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
                    }
                    index += self.d.len();
                    for a_index in 0..8 {
                        self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
                    }
                    Timer::after_nanos(63).await;
                    self.cs.set_low();
                    self.rd.set_low();
                    self.asout.set_low();
                    self.expand.set_low();

                    if is_svp {
                        self.pulse_clock(10);
                    }

                    Timer::after_nanos(375).await;

                    self.buffer[d] = 0;
                    self.buffer[d + 1] = 0;
                    for (index, pin) in self.d_snes.iter().enumerate() {
                        let true_index = if index < 2 {index} else {index+1} ;
                        self.buffer[d] |= (pin.is_high() as u8) << true_index;
                    }
                    self.buffer[d] |= (self.ciram_a10.is_high() as u8) << 2;
                    for (index, pin) in self.a[8..15].iter().enumerate() {
                        self.buffer[d + 1] |= (pin.is_high() as u8) << index;
                    }
                    self.buffer[d + 1] |= (self.a15.is_high() as u8) << 7;

                    self.cs.set_high();
                    self.rd.set_high();
                    self.asout.set_high();
                    self.expand.set_high();

                    if is_svp {
                        self.pulse_clock(10);
                    }
                    d += 2;
                }
                self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
            }
        }
        if snk_mode == 3 {
            for curr_buffer in (0..cart_size_lockon / 2).step_by(self.buffer.len() / 2) {
                d = 0;
                for curr_word in 0..self.buffer.len()/2 {
                    let my_address = curr_buffer + curr_word as u32 + (cart_size + cart_size_lockon) / 2;
                    let mut index = 0;
                    self.m2.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.pgr_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.chr_wr.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.ciram_ce.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.a[15].set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.chr_rd.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.irq.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    self.prg_rw.set_level(Level::from((my_address & (1 << index)) > 0));
                    index += 1;
                    for d_index in 0..self.d.len() {
                        self.d[d_index].set_level(Level::from((my_address & (1 << (index + d_index))) > 0));
                    }
                    index += self.d.len();
                    for a_index in 0..8 {
                        self.a[a_index].set_level(Level::from((my_address & (1 << (index + a_index))) > 0));
                    }
                    Timer::after_nanos(63).await;
                    self.cs.set_low();
                    self.rd.set_low();
                    self.asout.set_low();
                    self.expand.set_low();

                    if is_svp {
                        self.pulse_clock(10);
                    }

                    Timer::after_nanos(375).await;

                    self.buffer[d] = 0;
                    self.buffer[d + 1] = 0;
                    for (index, pin) in self.d_snes.iter().enumerate() {
                        let true_index = if index < 2 {index} else {index+1} ;
                        self.buffer[d] |= (pin.is_high() as u8) << true_index;
                    }
                    self.buffer[d] |= (self.ciram_a10.is_high() as u8) << 2;
                    for (index, pin) in self.a[8..15].iter().enumerate() {
                        self.buffer[d + 1] |= (pin.is_high() as u8) << index;
                    }
                    self.buffer[d + 1] |= (self.a15.is_high() as u8) << 7;

                    self.cs.set_high();
                    self.rd.set_high();
                    self.asout.set_high();
                    self.expand.set_high();

                    if is_svp {
                        self.pulse_clock(1);
                    }
                    d += 2;
                }
                self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
            }
        }

        if cart_size > 0x400000 {
            self.write_ssf2_map(0x50987E, 6).await;
            self.write_ssf2_map(0x50987F, 7).await;
        }
    }
}