use ch32_hal::{gpio::{Flex, Input, Level, Output, Pin, Pull}, Peripheral};
use embassy_time::Timer;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub const DATA_CHANNEL_SIZE: usize = 32;
pub enum Msg {
    Start,
    DumpSetupData(u8, u8, u8),
    Data([u8; DATA_CHANNEL_SIZE]),
    End,
}


pub struct DumperClass<'d> {
    m2: Output<'d>,
    pgr_ce: Output<'d>,
    //chr_wr: Output<'d>,
    //ciram_ce: Input<'d>,
    chr_rd: Output<'d>,
    //irq: Input<'d>,
    prg_rw: Output<'d>,
    a: [Output<'d>; 16],
    //ciram_a10: Input<'d>,
    d: [Flex<'d>; 8],
    in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
    buffer: &'d mut [u8; DATA_CHANNEL_SIZE],
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
        in_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        out_channel: &'d Channel<CriticalSectionRawMutex, Msg, 1>,
        buffer: &'d mut [u8; DATA_CHANNEL_SIZE],
    ) -> Self {
        let m2 = Output::new(m2_pin, Level::High, Default::default());
        let pgr_ce = Output::new(pgr_ce_pin, Level::High, Default::default());
        Output::new(chr_wr_pin, Level::High, Default::default()); // let chr_wr = 
        Input::new(ciram_ce_pin, Pull::Up); // let ciram_ce = 
        let chr_rd = Output::new(chr_rd_pin, Level::High, Default::default());
        Input::new(irq_pin, Pull::Up); // let irq = 
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

        Input::new(ciram_a10_pin, Pull::Up); // let ciram_a10 = 

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

       return Self { 
            m2, 
            pgr_ce, 
            //chr_wr, 
            //ciram_ce, 
            chr_rd, 
            //irq,
            prg_rw,
            a,
            //ciram_a10,
            d,
            in_channel,
            out_channel,
            buffer,
        }
    }

    fn set_address(&mut self, address: u16) {
        for index in 0..self.a.len() - 1 {
            self.a[index].set_level(Level::from((address & (1 << index)) > 0));
        }
        // PPU /A13
        self.a[self.a.len()-1].set_level(Level::from((address & (1 << 13)) == 0));
    }

    fn set_read_mode(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_as_input(Pull::Up);
        }
    }

    fn set_write_mode(&mut self) {
        for pin in self.d.iter_mut() {
            pin.set_as_output(Default::default());
            pin.set_low();
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
        //  _delay_us(1);
        self.set_phy2_high();
        //_delay_us(10);
        self.set_romsel(address);  // ROMSEL is low if need, PHI2 high
        Timer::after_micros(1).await;  // WRITING
        //_delay_ms(1); // WRITING
        // PHI2 low, ROMSEL high
        self.set_phy2_low();
        Timer::after_micros(1).await;  // WRITING
        self.set_romsel_high();
        // Back to read mode
        //  _delay_us(1);
        self.set_prg_read();
        self.set_read_mode();
        self.set_address(0);
        // Set phi2 to high state to keep cartridge unreseted
        //  _delay_us(1);
        self.set_phy2_high();
        //  _delay_us(1);
    }

    async fn read_prg_byte(&mut self, address: u16) -> u8 {
        self.set_read_mode();
        self.set_prg_read();
        self.set_romsel_high();
        self.set_address(address);
        self.set_phy2_high();
        self.set_romsel(address);
        Timer::after_micros(1).await;
        self.read_data()
    }

    async fn read_chr_byte(&mut self, address: u16) -> u8 {
        self.set_read_mode();
        self.set_phy2_high();
        self.set_romsel_high();
        self.set_address(address);
        self.set_chr_read_low();
        Timer::after_micros(1).await;
        let result = self.read_data();
        self.set_chr_read_high();
        result
    }

    async fn dump_prg(&mut self, base: u16, address: u16) {
        for x in 0..self.buffer.len() {
             self.buffer[x] = self.read_prg_byte(base + address + x as u16).await;
        }
        self.out_channel.send(Msg::Data(*self.buffer)).await;
    }

    async fn dump_chr(&mut self, address: u16) {
        for x in 0..self.buffer.len() {
            self.buffer[x] = self.read_chr_byte(address + x as u16).await;
        }
        self.out_channel.send(Msg::Data(*self.buffer)).await;
    }

    async fn dump_bank_prg(&mut self, from: u16, to: u16, base: u16) {
        for address in (from..to).step_by(DATA_CHANNEL_SIZE) {
            self.dump_prg(base, address).await;
        }
    }

    async fn dump_bank_chr(&mut self, from: u16, to: u16) {
        for address in (from..to).step_by(DATA_CHANNEL_SIZE) {
            self.dump_chr(address).await;
        }
    }

    pub async fn dump(&mut self) {
        for dpin in &mut self.d {
            dpin.set_as_input(Pull::Up);
        }
        
        /*
        let crc32 =    unsafe { &mut *CRC32.0.get() };
        let crc32_mmc3 =    unsafe { &mut *CRC32_MMC3.0.get() };

        for c in 0..512 {
            crc32[c] = read_prg_byte(u16::try_from(0x8000 + c).expect("address overflow"),&mut (&mut a, &mut d, &mut prg_rw, &mut pgr_ce, &mut m2)).await;
            crc32_mmc3[c] = read_prg_byte(u16::try_from(0xE000 + c).expect("address overflow"),&mut (&mut a, &mut d, &mut prg_rw, &mut pgr_ce, &mut m2)).await;
        } 

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

        let prgsize = 1;
        let chrsize = 1;
        let ramsize = 0;
        */
        let mapper = 4;
        let prg_banks = 4;
        let chr_banks = 5;

        let prg = (1 << prg_banks) * 16; // 2^prgsize * 16
        let chr = (1 << chr_banks) * 4; // 2^chrsize * 4
        //let ram = 0; // 0

        let receiver = self.in_channel.receiver();
        loop {
            match receiver.receive().await {
                Msg::Start => {
                    self.out_channel.send(Msg::DumpSetupData(mapper, prg, chr)).await;

                    self.read_prg(mapper, prg_banks).await;
                    self.read_chr(mapper, chr_banks).await;
                    self.out_channel.send(Msg::End).await;
                }
                _ => {}
            }
        }
    }

    async fn read_prg(&mut self, mapper: u8, banks: u8) {
        self.set_address(0);
        Timer::after_micros(1).await;
        match mapper {
            0 => {
                let base: u16 = 0x8000;
                self.dump_bank_prg(0x0, 0x4000 * (banks as u16), base).await;
            },
            4 => {
                //banks = int_pow(2, prgsize) * 2;
                self.write_prg_byte(0xA001, 0x80).await;  // Block Register - PRG RAM Chip Enable, Writable
                for i in 0..banks {
                    self.write_prg_byte(0x8000, 0x06).await;  // PRG Bank 0 ($8000-$9FFF)
                    self.write_prg_byte(0x8001, i).await;
                    self.dump_bank_prg(0x0, 0x2000, 0x8000).await;
                }
            },
            _ => {}
        }
    }

    async fn read_chr(&mut self, mapper: u8, banks: u8) {
        self.set_address(0);
        Timer::after_micros(1).await;
        match mapper {
            0 => {
                self.dump_bank_chr(0x0, 0x2000).await;
            },
            4 => {
                self.write_prg_byte(0xA001, 0x80).await;
                for i in 0..banks {
                    self.write_prg_byte(0x8000, 0x02).await;
                    self.write_prg_byte(0x8001, i).await;
                    self.dump_bank_chr(0x1000, 0x1400).await;
                }                
            }
            _ => {}
        }
    }
}