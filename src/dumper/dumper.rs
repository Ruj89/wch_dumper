use ch32_hal::{gpio::{Flex, Input, Level, Output, Pin, Pull}, Peripheral};
use embassy_time::Timer;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub const BYTE_READ_RETRIES: usize = 1;

pub enum MsgStartConsole {
    Nes,
    Snes,
}

impl Msg {
    pub const DATA_CHANNEL_SIZE: usize = 32;
    pub const DUMP_SETUP_DATA_CHANGED_LENGTH: usize = Msg::DATA_CHANNEL_SIZE / 2;
}

pub enum Msg {
    Start {
        console: MsgStartConsole
    },
    DumpSetupData {
        rom_size: u32,
    },
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

pub struct DumperConfig {
    pub mapper: u8,
    pub prgsize: u8,
    pub chrsize: u8,
    pub prg: u16, // KB
    pub chr: u16, // KB
}

#[repr(u8)]
pub enum SnesRomType {
    LO = 0,
    HI = 1,
    SA = 3,
    EX = 4,
}
pub struct DumperClass<'d> {
    m2: Output<'d>,
    pgr_ce: Output<'d>,
    chr_wr: Output<'d>,
    ciram_ce: Flex<'d>,
    chr_rd: Output<'d>,
    irq: Flex<'d>,
    prg_rw: Output<'d>,
    a: [Output<'d>; 16],
    ciram_a10: Flex<'d>,
    d: [Flex<'d>; 8],
    a15: Output<'d>,
    reset: Output<'d>,
    cs: Output<'d>,
    wr: Output<'d>,
    rd: Output<'d>,
    refresh: Output<'d>,
    expand: Input<'d>,
    d_snes: [Flex<'d>; 7],
    irq_snes: Input<'d>,
    in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    buffer: &'d mut [u8; Msg::DATA_CHANNEL_SIZE],
    config: DumperConfig,
}

impl<'d> DumperClass<'d>
{
    pub fn new(
        m2_pin: impl Peripheral<P = impl Pin> + 'd,
        pgr_ce_pin: impl Peripheral<P = impl Pin> + 'd,
        chr_wr_pin: impl Peripheral<P = impl Pin> + 'd,
        ciram_ce_pin: impl Peripheral<P = impl Pin> + 'd,
        chr_rd_pin: impl Peripheral<P = impl Pin> + 'd,
        irq_pin: impl Peripheral<P = impl Pin> + 'd,
        prg_rw_pin: impl Peripheral<P = impl Pin> + 'd,
        a_pins: (
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
        ),
        ciram_a10_pin: impl Peripheral<P = impl Pin> + 'd,
        d_pins: (
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
        ),
        a15_pin: impl Peripheral<P = impl Pin> + 'd,
        reset_pin: impl Peripheral<P = impl Pin> + 'd,
        cs_pin: impl Peripheral<P = impl Pin> + 'd,
        wr_pin: impl Peripheral<P = impl Pin> + 'd,
        rd_pin: impl Peripheral<P = impl Pin> + 'd,
        refresh_pin: impl Peripheral<P = impl Pin> + 'd,
        expand_pin: impl Peripheral<P = impl Pin> + 'd,
        d_snes_pins: (
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
            impl Peripheral<P = impl Pin> + 'd,
        ),
        irq_snes_pin: impl Peripheral<P = impl Pin> + 'd,
        in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        buffer: &'d mut [u8; Msg::DATA_CHANNEL_SIZE],
    ) -> Self {
        let m2 = Output::new(m2_pin, Level::High, Default::default());
        let pgr_ce = Output::new(pgr_ce_pin, Level::High, Default::default());
        let chr_wr = Output::new(chr_wr_pin, Level::High, Default::default());
        let ciram_ce = Flex::new(ciram_ce_pin);
        let chr_rd = Output::new(chr_rd_pin, Level::High, Default::default());
        let irq: Flex<'_> = Flex::new(irq_pin);
        let prg_rw = Output::new(prg_rw_pin, Level::High, Default::default());

        let a = [
            Output::new(a_pins.0, Level::Low, Default::default()),
            Output::new(a_pins.1, Level::Low, Default::default()),
            Output::new(a_pins.2, Level::Low, Default::default()),
            Output::new(a_pins.3, Level::Low, Default::default()),
            Output::new(a_pins.4, Level::Low, Default::default()),
            Output::new(a_pins.5, Level::Low, Default::default()),
            Output::new(a_pins.6, Level::Low, Default::default()),
            Output::new(a_pins.7, Level::Low, Default::default()),
            Output::new(a_pins.8, Level::Low, Default::default()),
            Output::new(a_pins.9, Level::Low, Default::default()),
            Output::new(a_pins.10, Level::Low, Default::default()),
            Output::new(a_pins.11, Level::Low, Default::default()),
            Output::new(a_pins.12, Level::Low, Default::default()),
            Output::new(a_pins.13, Level::Low, Default::default()),
            Output::new(a_pins.14, Level::Low, Default::default()),
            Output::new(a_pins.15, Level::High, Default::default()),
        ];

        let ciram_a10 = Flex::new(ciram_a10_pin);

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

        let a15 = Output::new(a15_pin, Level::High, Default::default());
        let reset = Output::new(reset_pin, Level::High, Default::default());
        let cs = Output::new(cs_pin, Level::High, Default::default());
        let wr: Output<'_> = Output::new(wr_pin, Level::High, Default::default());
        let rd: Output<'_> = Output::new(rd_pin, Level::High, Default::default());
        let refresh = Output::new(refresh_pin, Level::High, Default::default());
        let expand = Input::new(expand_pin, Pull::None);

        let d_snes = [
            Flex::new(d_snes_pins.0),
            Flex::new(d_snes_pins.1),
            Flex::new(d_snes_pins.2),
            Flex::new(d_snes_pins.3),
            Flex::new(d_snes_pins.4),
            Flex::new(d_snes_pins.5),
            Flex::new(d_snes_pins.6),
        ];
        let irq_snes = Input::new(irq_snes_pin, Pull::None);

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
        let config = DumperConfig {
            mapper: 1,
            prgsize: 3,
            chrsize: 0,
            prg: 128,
            chr: 0
        };

       return Self {
            m2,
            pgr_ce,
            chr_wr,
            ciram_ce,
            chr_rd,
            irq,
            prg_rw,
            a,
            ciram_a10,
            d,
            a15,
            reset,
            cs,
            wr,
            rd,
            refresh,
            expand,
            d_snes,
            irq_snes,
            in_channel,
            out_channel,
            buffer,
            config,
        }
    }

    fn set_address(&mut self, address: u16) {
        for index in 0..self.a.len() - 1 {
            self.a[index].set_level(Level::from((address & (1 << index)) > 0));
        }
        // PPU /A13
        self.a[self.a.len()-1].set_level(Level::from((address & (1 << 13)) == 0));
    }

    fn set_mode_read(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_as_input(Pull::Up);
        }
    }

    fn set_write_mode(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_low();
            pin.set_as_output(Default::default());
        }
    }

    fn set_prg_read(&mut self){
        self.prg_rw.set_high();
    }

    fn set_prg_write(&mut self){
        self.prg_rw.set_low();
    }

    fn set_romsel_low(&mut self){
        self.pgr_ce.set_low();
    }

    fn set_romsel_high(&mut self){
        self.pgr_ce.set_high();
    }

    fn set_romsel(&mut self, address: u16) {
    if address & 0x8000 > 0 {
        self.set_romsel_low();
    } else {
        self.set_romsel_high();
    }
    }

    fn set_phy2_high(&mut self){
        self.m2.set_high();
    }

    fn set_phy2_low(&mut self){
        self.m2.set_low();
    }

    fn set_chr_read_high(&mut self){
        self.chr_rd.set_high();
    }

    fn set_chr_read_low(&mut self){
        self.chr_rd.set_low();
    }


    fn set_romsel_low_and_m2_high(&mut self){
        self.m2.set_high();
        self.pgr_ce.set_low();
    }

    fn set_romsel_high_and_m2_low(&mut self){
        self.m2.set_low();
        self.pgr_ce.set_high();
    }

    fn read_data(&mut self) -> u8{
        let mut data = 0;
        for (index, pin) in self.d.iter().enumerate() {
            data |= (pin.is_high() as u8) << index;
        }
        data
    }

    fn write_data(&mut self, data: u8){
        for (index, pin) in self.d.iter_mut().enumerate() {
            pin.set_level(Level::from((data & (1 << index)) > 0));
        }
    }

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

    async fn dump_prg(&mut self, base: u16, address: u16) {
        for x in 0..self.buffer.len() {
             self.buffer[x] = self.read_prg_byte(base + address + x as u16).await;
        }
        self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
    }

    async fn dump_chr(&mut self, address: u16) {
        for x in 0..self.buffer.len() {
            self.buffer[x] = self.read_chr_byte(address + x as u16).await;
        }
        self.out_channel.send(Msg::Data{data: *self.buffer, length: self.buffer.len()}).await;
    }

    async fn dump_bank_prg(&mut self, from: u16, to: u16, base: u16) {
        for address in (from..to).step_by(Msg::DATA_CHANNEL_SIZE) {
            self.dump_prg(base, address).await;
        }
    }

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
                        MsgStartConsole::Nes => {self.dump_nes().await;}
                        MsgStartConsole::Snes => {self.dump_snes().await;}
                    };
                }
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

    async fn dump_nes(&mut self) {
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

    fn set_address_b(&mut self, address: u8) {
        for index in 0..8 {
            self.a[index].set_level(Level::from((address & (1 << (index))) > 0));
        }
    }

    fn set_address_p(&mut self, address: u8) {
        for index in 8..15 {
            self.a[index].set_level(Level::from((address & (1 << (index-8))) > 0));
        }
        self.a15.set_level(Level::from((address & (1 << 7)) > 0));
    }

    fn set_d_snes_pullup(&mut self) {
        for index in 0..7 {
            self.d_snes[index].set_as_input(Pull::Up);
        }
        self.ciram_a10.set_as_input(Pull::Up);
    }

    fn read_snes_data(&mut self) -> u8 {
        let mut data = 0;
        for (index, pin) in self.d_snes.iter().enumerate() {
            let true_index = if index < 2 {index} else {index+1} ;
            data |= (pin.is_high() as u8) << true_index;
        }
        data |= (self.ciram_a10.is_high() as u8) << 2;
        data
    }

    fn set_reset_high(&mut self){
        self.reset.set_high();
    }

    fn set_reset_low(&mut self){
        self.reset.set_low();
    }

    fn set_wr_high(&mut self){
        self.wr.set_high();
    }

    fn set_wr_low(&mut self){
        self.wr.set_low();
    }

    fn set_rd_high(&mut self){
        self.rd.set_high();
    }

    fn set_rd_low(&mut self){
        self.rd.set_low();
    }

    fn set_cs_high(&mut self){
        self.cs.set_high();
    }

    fn set_cs_low(&mut self){
        self.cs.set_low();
    }

    fn set_refresh_high(&mut self){
        self.refresh.set_high();
    }

    fn set_refresh_low(&mut self){
        self.refresh.set_low();
    }

    fn data_in(&mut self) {
        self.set_d_snes_pullup();
    }

    fn control_in_snes(&mut self) {
        self.set_wr_high();
        self.set_cs_low();
        self.set_rd_low();
    }

    async fn dump_snes(&mut self) {
        self.ciram_ce.set_as_output(Default::default());
        self.ciram_ce.set_low();
        self.irq.set_as_output(Default::default());
        self.irq.set_low();
        for d_index in 0..8 {
            self.d[d_index].set_as_output(Default::default());
            self.d[d_index].set_low();
        }

        self.set_reset_high();
        self.set_wr_high();
        self.set_cs_low();
        self.set_rd_low();

        self.set_refresh_low();

        let (num_banks, rom_type) = self.get_cart_info_snes().await;
        let rom_size = match rom_type {
            v if v == SnesRomType::LO as u8 => {(0x10000 - 0x8000) * num_banks as u32},
            v if v == SnesRomType::HI as u8 => {0x10000 * num_banks as u32},
            _ => {0}
        };
        self.out_channel.send(Msg::DumpSetupData{ rom_size }).await;
        self.read_rom_snes(num_banks, rom_type).await;
        self.out_channel.send(Msg::End).await;
    }

    async fn get_cart_info_snes(&mut self) -> (u8, u8) {
        self.set_address_b(0b11000000);
        for curr_byte in 0..1024 {
            self.set_address_a(curr_byte);
            Timer::after_nanos(375).await;
        }
        self.check_cart_snes().await
    }

    async fn check_cart_snes(&mut self) -> (u8, u8) {
        self.data_in();

        let header_start = 0xFFB0;
        let mut snes_header = [0u8;80];
        self.set_address_b(0x00);
        for c in 0..80 {
            let curr_byte = header_start + c as u16;
            self.set_address_a(curr_byte);
            Timer::after_nanos(750).await;

            snes_header[c] = self.read_snes_data();
        }
        let rom_type = match snes_header[(0xFFD5 - header_start) as usize] {
            0x35 => {SnesRomType::EX as u8},
            0x3A  => {SnesRomType::HI as u8},
            v if ((v >> 5) != 1) => {SnesRomType::LO as u8},
            v => {v & 1},
        };

        let rom_size_exp = snes_header[(0xFFD7 - header_start) as usize] - 7;
        let mut rom_size = 1;
        for _ in 0..rom_size_exp {
            rom_size *= 2;
        }

        (((rom_size as usize * 1024 * 1024 / 8) / (0x8000 + (rom_type as usize * 0x8000))) as u8, rom_type)
    }

    async fn read_rom_snes(&mut self, num_banks: u8, rom_type: u8) {
        self.data_in();
        self.control_in_snes();
        match rom_type {
            v if v == SnesRomType::LO as u8 =>  {self.read_lo_rom_banks(0, num_banks).await;}
            v if v == SnesRomType::HI as u8 =>  {self.read_hi_rom_banks(192, num_banks + 192).await;}
            _ => {}
        }
    }

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

    async fn read_hi_rom_banks(&mut self, start: u8, end: u8) {
        for curr_bank in start..end {
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
}