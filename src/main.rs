use std::fmt::{Display, Error, Formatter};
use std::fs;
use std::io::{stdout, Write};
use std::path::Path;
use std::{env, time};
use termion::raw::IntoRawMode;

struct Cpu {
    pc: u16,
    i: u16,
    v: [u8; Cpu::NUM_REGISTERS],
    delay_timer: u8,
    sound_timer: u8,
    stack: [u16; Cpu::MAX_STACK],
    sp: usize,
    ram: [u8; Cpu::RAM_SIZE],
    vram: [u8; Cpu::VRAM_SIZE],
}

impl Cpu {
    const PC_START: usize = 0x200;
    const OP_SIZE: u16 = 2;
    const RAM_SIZE: usize = 4096;
    const VRAM_WIDTH: usize = 64;
    const VRAM_HEIGHT: usize = 32;
    const VRAM_SIZE: usize = Cpu::VRAM_HEIGHT * Cpu::VRAM_WIDTH;
    const NUM_REGISTERS: usize = 16;
    const MAX_STACK: usize = 24;
    const FONT_SET: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];

    pub fn new() -> Self {
        let mut cpu = Cpu {
            pc: Cpu::PC_START as u16,
            i: 0,
            v: [0; Cpu::NUM_REGISTERS],
            delay_timer: 0,
            sound_timer: 0,
            stack: [0; Cpu::MAX_STACK],
            sp: 0,
            ram: [0; Cpu::RAM_SIZE],
            vram: [0; Cpu::VRAM_SIZE],
        };
        cpu.ram[..Cpu::FONT_SET.len()].copy_from_slice(&Cpu::FONT_SET);
        cpu
    }

    pub fn load_rom(&mut self, rom: &[u8]) {
        self.ram[Cpu::PC_START..][..rom.len()].copy_from_slice(rom);
    }

    pub fn run(&mut self) {
        let mut timer: usize = 0;
        let mut stdout = stdout().into_raw_mode().unwrap();
        let mut frame = String::new();
        frame.reserve(Cpu::VRAM_SIZE + Cpu::VRAM_HEIGHT);
        loop {
            let op = self.fetch_op();
            self.decode_op(op);
            //println!("{}", self);
            if timer % 10 == 0 {
                self.update_timers();
            }
            timer += 1;

            frame.clear();
            for y in 0..Cpu::VRAM_HEIGHT {
                for x in 0..Cpu::VRAM_WIDTH {
                    let pix = if self.vram[x + y * Cpu::VRAM_WIDTH] == 1 {
                        '█'
                    } else {
                        ' '
                    };
                    frame.push(pix);
                }

                frame.push_str("\r\n");
            }
            write!(
                stdout,
                "{}{}{}",
                termion::clear::All,
                termion::cursor::Hide,
                frame
            )
            .unwrap();
            stdout.flush().unwrap();
            std::thread::sleep(time::Duration::from_millis(1000 / 600))
        }
    }

    fn fetch_op(&mut self) -> (u8, usize, usize, usize) {
        let pc = self.pc as usize;
        let b1 = self.ram[pc];
        let b2 = self.ram[pc + 1];

        self.pc += Cpu::OP_SIZE;

        (
            (b1 & 0xF0) >> 4,
            (b1 & 0x0F) as usize,
            ((b2 & 0xF0) >> 4) as usize,
            (b2 & 0x0F) as usize,
        )
    }

    fn decode_op(&mut self, op: (u8, usize, usize, usize)) {
        match op {
            (0x0, 0x0, 0xE, 0x0) => self.vram = [0; Cpu::VRAM_SIZE],
            (0x0, 0x0, 0xE, 0xE) => {
                self.sp -= 1;
                self.pc = self.stack[self.sp];
            }
            (0x0, _, _, _) => unimplemented!("Deprecated op"),
            (0x1, n1, n2, n3) => self.pc = Cpu::n3u16(n1, n2, n3),
            (0x2, n1, n2, n3) => {
                self.stack[self.sp] = self.pc;
                self.sp += 1;
                self.pc = Cpu::n3u16(n1, n2, n3);
            }
            (0x3, vx, n1, n2) => {
                if self.v[vx] == Cpu::n2u8(n1, n2) {
                    self.pc += Cpu::OP_SIZE;
                }
            }
            (0x4, vx, n1, n2) => {
                if self.v[vx] != Cpu::n2u8(n1, n2) {
                    self.pc += Cpu::OP_SIZE;
                }
            }
            (0x5, vx, vy, 0x0) => {
                if self.v[vx] == self.v[vy] {
                    self.pc += Cpu::OP_SIZE;
                }
            }
            (0x6, vx, n1, n2) => self.v[vx] = Cpu::n2u8(n1, n2),
            (0x7, vx, n1, n2) => self.v[vx] += Cpu::n2u8(n1, n2),
            (0x8, vx, vy, 0x0) => self.v[vx] = self.v[vy],
            (0x8, vx, vy, 0x1) => self.v[vx] |= self.v[vy],
            (0x8, vx, vy, 0x2) => self.v[vx] &= self.v[vy],
            (0x8, vx, vy, 0x3) => self.v[vx] ^= self.v[vy],
            (0x8, vx, vy, 0x4) => {
                let res = self.v[vx] as u16 + self.v[vy] as u16;
                self.v[0xF] = if res > 0xFF { 1 } else { 0 };
                self.v[vx] = res as u8;
            }
            (0x8, vx, vy, 0x5) => {
                let res = self.v[vx] as i8 - self.v[vy] as i8;
                self.v[0xF] = if res < 0 { 1 } else { 0 };
                self.v[vx] = res as u8;
            }
            (0x8, vx, _, 0x6) => {
                self.v[0xF] = self.v[vx] & 0x01;
                self.v[vx] >>= 1;
            }
            (0x8, vx, _, 0xE) => {
                self.v[0xF] = self.v[vx] & 0x80;
                self.v[vx] <<= 1;
            }
            (0x9, vx, vy, 0x0) => {
                if self.v[vx] != self.v[vy] {
                    self.pc += Cpu::OP_SIZE;
                }
            }
            (0xA, n1, n2, n3) => self.i = Cpu::n3u16(n1, n2, n3),
            (0xB, n1, n2, n3) => self.pc = Cpu::n3u16(n1, n2, n3) + u16::from(self.v[0]),
            (0xC, vx, n1, n2) => self.v[vx] = rand::random::<u8>() & Cpu::n2u8(n1, n2),
            (0xD, vx, vy, n) => {
                let sprite = &self.ram[self.i as usize..][..n];
                let x = self.v[vx] as usize;
                let y = self.v[vy] as usize;

                self.v[0xF] = 0;
                for h in 0..sprite.len() {
                    for w in 0..8 {
                        let pix = (sprite[h] >> (7 - w)) & 0x01;
                        let pos = (x + w) % Cpu::VRAM_WIDTH
                            + ((y + h) % Cpu::VRAM_HEIGHT) * Cpu::VRAM_WIDTH;
                        self.v[0xF] |= self.vram[pos] & pix;
                        self.vram[pos] ^= pix;
                    }
                }
            }
            (0xE, vx, 0x9, 0xE) => {
                //                if let Key::Char(c) = stdin().keys().next().unwrap().unwrap() {
                //                    if c == (self.v[vx] + 48) as char {
                //                        self.pc += Cpu::OP_SIZE;
                //                    }
                //                }
            }
            (0xE, vx, 0xA, 0x1) => {
                //                if let Key::Char(c) = stdin().keys().next().unwrap().unwrap() {
                //                    if c == (self.v[vx] + 48) as char {
                //                        self.pc -= Cpu::OP_SIZE;
                //                    }
                //                }
                self.pc += Cpu::OP_SIZE;
            }
            (0xF, vx, 0x0, 0x7) => self.v[vx] = self.delay_timer,
            (0xF, vx, 0x1, 0x5) => self.delay_timer = self.v[vx],
            (0xF, vx, 0x1, 0x8) => self.sound_timer = self.v[vx],
            (0xF, vx, 0x1, 0xE) => self.i += u16::from(self.v[vx]),
            (0xF, n, 0x2, 0x9) => self.i = n as u16 * 5,
            (0xF, vx, 0x3, 0x3) => {
                let v = self.v[vx];
                self.ram[self.i as usize] = v / 100;
                self.ram[(self.i + 1) as usize] = (v / 10) % 10;
                self.ram[(self.i + 2) as usize] = v % 10;
            }
            (0xF, vx, 0x5, 0x5) => {
                self.ram[self.i as usize..][0..=vx].copy_from_slice(&self.v[0..=vx])
            }

            (0xF, vx, 0x6, 0x5) => {
                self.v[0..=vx].copy_from_slice(&self.ram[self.i as usize..][0..=vx])
            }

            _ => panic!("Invalid op: {:X?}", op),
        }
    }

    fn update_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    fn n2u8(n1: usize, n2: usize) -> u8 {
        (n1 << 4 | n2) as u8
    }
    fn n3u16(n1: usize, n2: usize, n3: usize) -> u16 {
        (n1 << 8 | n2 << 4 | n3) as u16
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        writeln!(
            f,
            "PC: {}\nI: {}\nV: {:?}\nDelay timer: {}\nSound timer: {}\nStack: {:?}\nSp: {}",
            self.pc, self.i, self.v, self.delay_timer, self.sound_timer, self.stack, self.sp
        )
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rustichip8 rom.ch8");
        return;
    }

    let rom = Path::new(args[1].as_str());
    let rom_data = fs::read(rom).unwrap();
    let mut cpu = Cpu::new();
    cpu.load_rom(rom_data.as_slice());
    cpu.run();
}
