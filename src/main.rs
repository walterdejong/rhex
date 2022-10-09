/*
    rhex    WJ122

    * a rusty hex viewer
*/

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent};
use crossterm::style::Stylize;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::tty::IsTty;
use crossterm::{cursor, execute, style, terminal, QueueableCommand};
use float_pretty_print::PrettyPrintFloat;
use std::env::{self};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Write as fmtWrite;
use std::fs::File;
use std::io::Write as ioWrite;
use std::io::{stdout, Read, Seek, Stdout};
use std::path::Path;
use std::process;

use Endiannes::*;

#[derive(Debug, PartialEq, Eq)]
enum Endiannes {
    LittleEndian,
    BigEndian,
}

const HEX_PAGESIZE: usize = 1024;

#[derive(Debug)]
#[allow(dead_code)]
struct HexView {
    stdout: Stdout,
    terminal_width: u16,
    terminal_height: u16,

    view_width: u16,
    view_height: u16,
    leftpane_width: u16,
    centerpane_width: u16,
    rightpane_width: u16,

    cursor_x: u16,
    cursor_y: u16,
    endian: Endiannes,

    filename: Option<String>,
    filesize: u64,
    fd: Option<File>,
    offset: u64,
    page_address: u64,
    page: [u8; HEX_PAGESIZE],

    update_needed: bool,
}

impl HexView {
    fn new() -> Self {
        let terminal_size = terminal::size().expect("unable to get terminal size");

        if terminal_size.0 < 80 {
            eprintln!("error: terminal is not wide enough");
            process::exit(1);
        }
        let view_width = 80;

        if terminal_size.1 < 10 {
            eprintln!("error: terminal is not high enough");
            process::exit(1);
        }
        // the hexdump view will be most of the screen
        // we need 6 lines at the bottom for the info pane
        let view_height = terminal_size.1 - 6;

        HexView {
            stdout: stdout(),
            terminal_width: terminal_size.0,
            terminal_height: terminal_size.1,
            view_width,
            view_height,
            leftpane_width: 10,   // address: 8 + spacing: 2
            centerpane_width: 50, // hex bytes: 8 * (2 + 1) * 2 + spacing: 2
            rightpane_width: 17,  // ascii: 16 + spacing: 1
            cursor_x: 0,
            cursor_y: 0,
            endian: LittleEndian,
            filename: None,
            filesize: 0,
            fd: None,
            offset: 0,
            page_address: 0,
            page: [0u8; HEX_PAGESIZE],
            update_needed: false,
        }
    }

    fn load(&mut self, filename: &str) {
        self.fd = Some(
            File::open(filename)
                .with_context(|| format!("failed to open '{}'", filename))
                .unwrap(),
        );

        let metadata = std::fs::metadata(filename)
            .with_context(|| format!("failed to stat() file '{}'", filename))
            .unwrap();
        self.filesize = metadata.len();

        if self.filesize == 0 {
            eprintln!("empty file: {}", filename);
            process::exit(1);
        }

        if self.filesize > u32::MAX as u64 {
            // address will be printed extra-wide
            self.leftpane_width = 10 + 2;
        } else {
            // address will be printed with 8 hex digits
            self.leftpane_width = 8 + 2;
        }

        self.filename = Some(filename.to_owned());

        self.page_fault(0);
    }

    fn page_fault(&mut self, address: u64) {
        self.page_address = address / HEX_PAGESIZE as u64 * HEX_PAGESIZE as u64;

        self.page = [0; HEX_PAGESIZE]; // clear data buffer

        _ = self
            .fd
            .as_ref()
            .unwrap()
            .seek(std::io::SeekFrom::Start(self.page_address))
            .expect("seek error");
        _ = self
            .fd
            .as_ref()
            .unwrap()
            .read(&mut self.page)
            .expect("read() error");

        self.update_needed = true;
    }

    fn at(&mut self, address: u64) -> u8 {
        assert!(address < self.filesize);

        if address >= self.page_address && address < self.page_address + HEX_PAGESIZE as u64 {
            return self.page[(address - self.page_address) as usize];
        }

        self.page_fault(address);

        assert!(address >= self.page_address && address < self.page_address + HEX_PAGESIZE as u64);
        self.page[(address - self.page_address) as usize]
    }

    fn draw_screen(&mut self) {
        if !self.update_needed {
            return;
        }

        self.clearscreen();

        self.draw_hexdump();
        self.draw_bottom_pane();
        self.draw_cursor();

        self.stdout.flush().unwrap();
        self.update_needed = false;
    }

    fn clearscreen(&mut self) {
        self.stdout
            .queue(Clear(ClearType::All))
            .unwrap()
            .queue(cursor::MoveTo(0, 0))
            .unwrap();
    }

    fn draw_hexdump(&mut self) {
        for y in 0..self.view_height {
            self.draw_hexdump_line(y);
        }
    }

    fn draw_hexdump_line(&mut self, y: u16) {
        let mut linebuf = String::new();

        let addr = self.offset + y as u64 * 16;
        if addr >= self.filesize {
            return;
        }

        // left pane: address (also known as: offset)
        if self.filesize > u32::MAX as u64 {
            write!(linebuf, "{:10X}", addr).unwrap();
        } else {
            write!(linebuf, "{:08X}", addr).unwrap();
        }
        write!(linebuf, "  ").unwrap();

        // middle pane: hex bytes (left side: 8 bytes)
        for x in 0..8 {
            let offset = addr + x;
            if offset >= self.filesize {
                write!(linebuf, "   ").unwrap();
            } else {
                write!(linebuf, "{:02X} ", self.at(offset)).unwrap();
            }
        }
        write!(linebuf, " ").unwrap();

        // hex bytes (right side: 8 bytes)
        for x in 0..8 {
            let offset = addr + 8 + x;
            if offset >= self.filesize {
                write!(linebuf, "   ").unwrap();
            } else {
                write!(linebuf, "{:02X} ", self.at(offset)).unwrap();
            }
        }
        write!(linebuf, " ").unwrap();

        // right pane: character view (16 bytes)
        for x in 0..16 {
            let mut c;
            let offset = addr + x;
            if offset >= self.filesize {
                c = ' ';
            } else {
                c = self.at(offset) as char;
                if !(c >= ' ' && c <= '~') {
                    c = '.';
                }
            }
            linebuf.push(c);
        }
        linebuf.push(' ');

        self.stdout
            .queue(cursor::MoveTo(0, y as u16))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_bottom_pane(&mut self) {
        let y = self.view_height; // screen position
        let pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64;

        self.draw_info_address(y, pos);
        self.draw_info_i8(y + 1, pos);
        self.draw_info_i16(y + 2, pos);
        self.draw_info_i32(y + 3, pos);
        self.draw_info_i64(y + 4, pos);
        self.draw_info_f32_f64_and_endianness(y + 5, pos);
    }

    fn draw_info_address(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        if self.filesize > u32::MAX as u64 {
            write!(
                linebuf,
                "  @0x{:10x}  {:<10}  @{:<24}  size: {}",
                pos, " ", pos, self.filesize
            )
            .unwrap();
        } else {
            write!(
                linebuf,
                "  @0x{:08x}  {:<12}  @{:<24}  size: {} ",
                pos, " ", pos, self.filesize
            )
            .unwrap();
        }
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_info_i8(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        if pos < self.filesize {
            let data_i8 = self.at(pos) as i8;
            let data_u8 = self.at(pos);
            write!(
                linebuf,
                "  i8 : {:<20}  u8 : {:<20}  0x{:02x} ",
                data_i8, data_u8, data_u8
            )
            .unwrap();
        } else {
            write!(linebuf, "  i8 : {:<20}  u8 : {:<20}  --   ", "--", "--").unwrap();
        }
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_info_i16(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        if pos + 1 < self.filesize {
            let bytes16 = [self.at(pos), self.at(pos + 1)];
            let data_i16;
            let data_u16;
            if self.endian == LittleEndian {
                data_i16 = i16::from_le_bytes(bytes16);
                data_u16 = u16::from_le_bytes(bytes16);
            } else {
                data_i16 = i16::from_be_bytes(bytes16);
                data_u16 = u16::from_be_bytes(bytes16);
            }
            write!(
                linebuf,
                "  i16: {:<20}  u16: {:<20}  0x{:04x} ",
                data_i16, data_u16, data_u16
            )
            .unwrap();
        } else {
            write!(linebuf, "  i16: {:<20}  u16: {:<20}  --     ", "--", "--").unwrap();
        }
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_info_i32(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        let mut f32_value = String::new();

        if pos + 3 < self.filesize {
            let bytes32 = [
                self.at(pos),
                self.at(pos + 1),
                self.at(pos + 2),
                self.at(pos + 3),
            ];
            let data_i32;
            let data_u32;
            if self.endian == LittleEndian {
                data_i32 = i32::from_le_bytes(bytes32);
                data_u32 = u32::from_le_bytes(bytes32);
            } else {
                data_i32 = i32::from_be_bytes(bytes32);
                data_u32 = u32::from_be_bytes(bytes32);
            }
            write!(
                linebuf,
                "  i32: {:<20}  u32: {:<20}  0x{:08x} ",
                data_i32, data_u32, data_u32
            )
            .unwrap();

            let data_f32;
            if self.endian == LittleEndian {
                data_f32 = f32::from_le_bytes(bytes32);
            } else {
                data_f32 = f32::from_be_bytes(bytes32);
            }
            write!(f32_value, "{:20.20}", PrettyPrintFloat(data_f32 as f64)).unwrap();
        } else {
            write!(
                linebuf,
                "  i32: {:<20}  u32: {:<20}  --         ",
                "--", "--",
            )
            .unwrap();
        }
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_info_i64(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        if pos + 7 < self.filesize {
            let bytes64 = [
                self.at(pos),
                self.at(pos + 1),
                self.at(pos + 2),
                self.at(pos + 3),
                self.at(pos + 4),
                self.at(pos + 5),
                self.at(pos + 6),
                self.at(pos + 7),
            ];
            let data_i64;
            let data_u64;
            if self.endian == LittleEndian {
                data_i64 = i64::from_le_bytes(bytes64);
                data_u64 = u64::from_le_bytes(bytes64);
            } else {
                data_i64 = i64::from_be_bytes(bytes64);
                data_u64 = u64::from_be_bytes(bytes64);
            }
            write!(
                linebuf,
                "  i64: {:<20}  u64: {:<20}  0x{:016x} ",
                data_i64, data_u64, data_u64
            )
            .unwrap();
        } else {
            write!(
                linebuf,
                "  i64: {:<20}  u64: {:<20}  --                 ",
                "--", "--",
            )
            .unwrap();
        }
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn draw_info_f32_f64_and_endianness(&mut self, y: u16, pos: u64) {
        let mut linebuf = String::new();

        let mut f32_value = String::new();

        if pos + 3 < self.filesize {
            let bytes32 = [
                self.at(pos),
                self.at(pos + 1),
                self.at(pos + 2),
                self.at(pos + 3),
            ];
            let data_f32;
            if self.endian == LittleEndian {
                data_f32 = f32::from_le_bytes(bytes32);
            } else {
                data_f32 = f32::from_be_bytes(bytes32);
            }
            write!(f32_value, "{:20.20}", PrettyPrintFloat(data_f32 as f64)).unwrap();
        } else {
            write!(f32_value, "{}", "--").unwrap();
        }

        let mut f64_value = String::new();

        if pos + 7 < self.filesize {
            let bytes64 = [
                self.at(pos),
                self.at(pos + 1),
                self.at(pos + 2),
                self.at(pos + 3),
                self.at(pos + 4),
                self.at(pos + 5),
                self.at(pos + 6),
                self.at(pos + 7),
            ];

            let data_f64;
            if self.endian == LittleEndian {
                data_f64 = f64::from_le_bytes(bytes64);
            } else {
                data_f64 = f64::from_be_bytes(bytes64);
            }
            write!(f64_value, "{:20.20}", PrettyPrintFloat(data_f64)).unwrap();
        } else {
            write!(f64_value, "{}", "--").unwrap();
        }

        let s_endian;
        if self.endian == LittleEndian {
            s_endian = "little";
        } else {
            s_endian = "big";
        }
        write!(
            linebuf,
            "  f32: {:<20}  f64: {:<20}  {} endian   ",
            f32_value, f64_value, s_endian
        )
        .unwrap();
        self.stdout
            .queue(cursor::MoveTo(0, y))
            .unwrap()
            .queue(style::Print(&linebuf))
            .unwrap();
        linebuf.clear();
    }

    fn erase_cursor(&mut self) {
        // erase cursor via overdraw

        // cursor position in the hex dump view
        let mut xpos = self.leftpane_width + self.cursor_x * 3;
        if self.cursor_x >= 8 {
            xpos += 1;
        }
        let ypos = self.cursor_y;
        let data_pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64;

        let byte = self.at(data_pos);
        self.stdout
            .queue(cursor::MoveTo(xpos, ypos))
            .unwrap()
            .queue(style::Print(format!("{:02X}", byte)))
            .unwrap();

        // cursor position in right pane: ascii view
        xpos = self.leftpane_width + self.centerpane_width + self.cursor_x;

        let mut c = self.at(data_pos) as char;
        if !(c >= ' ' && c <= '~') {
            c = '.';
        }
        self.stdout
            .queue(cursor::MoveTo(xpos, ypos))
            .unwrap()
            .queue(style::Print(format!("{c}")))
            .unwrap();
    }

    fn draw_cursor(&mut self) {
        // draw cursor via overdraw

        // cursor position in the hex dump view
        let mut xpos = self.leftpane_width + self.cursor_x * 3;
        if self.cursor_x >= 8 {
            xpos += 1;
        }
        let ypos = self.cursor_y;
        let data_pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64;

        assert!(data_pos < self.filesize);

        let byte = self.at(data_pos);
        self.stdout
            .queue(cursor::MoveTo(xpos, ypos))
            .unwrap()
            .queue(style::PrintStyledContent(format!("{:02X}", byte).reverse()))
            .unwrap();

        // cursor position in right pane: ascii view
        xpos = self.leftpane_width + self.centerpane_width + self.cursor_x;

        let mut c = self.at(data_pos) as char;
        if !(c >= ' ' && c <= '~') {
            c = '.';
        }
        self.stdout
            .queue(cursor::MoveTo(xpos, ypos))
            .unwrap()
            .queue(style::PrintStyledContent(format!("{c}").reverse()))
            .unwrap();
    }

    fn key_event(&mut self, key_event: &KeyEvent) {
        match key_event.code {
            KeyCode::Right => self.key_right(),
            KeyCode::Left => self.key_left(),
            KeyCode::Up => self.key_up(),
            KeyCode::Down => self.key_down(),
            KeyCode::PageUp => self.key_pageup(),
            KeyCode::PageDown => self.key_pagedown(),
            KeyCode::Home => self.key_home(),
            KeyCode::End => self.key_end(),
            KeyCode::Char('e') => self.toggle_endianness(),
            KeyCode::Char('l') => self.key_little_endian(),
            KeyCode::Char('b') => self.key_big_endian(),
            _ => {}
        }
    }

    fn toggle_endianness(&mut self) {
        if self.endian == LittleEndian {
            self.endian = BigEndian;
        } else {
            self.endian = LittleEndian;
        }
        self.draw_bottom_pane();
        self.stdout.flush().unwrap();
    }

    fn key_little_endian(&mut self) {
        if self.endian == LittleEndian {
            return;
        }
        self.toggle_endianness();
    }

    fn key_big_endian(&mut self) {
        if self.endian == BigEndian {
            return;
        }
        self.toggle_endianness();
    }

    fn key_right(&mut self) {
        // cursor can not go beyond EOF
        let pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64 + 1;
        if pos >= self.filesize {
            return;
        }

        self.erase_cursor();

        self.cursor_x += 1;
        if self.cursor_x >= 16 {
            self.cursor_x = 0;
            self.cursor_y += 1;
            if self.cursor_y >= self.view_height {
                self.cursor_y = self.view_height - 1;
                // scroll
                self.offset += 16;
                self.update_needed = true;
            }
        }

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn key_left(&mut self) {
        let pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64;
        if pos == 0 {
            return;
        }

        self.erase_cursor();

        if self.cursor_x == 0 {
            if self.cursor_y == 0 {
                // scroll
                self.offset -= 16;
                self.update_needed = true;
            } else {
                self.cursor_y -= 1;
            }
            self.cursor_x = 15;
        } else {
            self.cursor_x -= 1;
        }

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn key_down(&mut self) {
        // cursor can not go beyond EOF
        let pos = self.offset + (self.cursor_y as u64 + 1) * 16 + self.cursor_x as u64;
        if pos >= self.filesize {
            // put cursor position at EOF
            let pos = (self.filesize - 1 - self.offset) as u16;
            let cy = pos / 16;
            let cx = pos % 16;

            if self.cursor_x != cx || self.cursor_y != cy {
                self.erase_cursor();
                self.cursor_x = cx;
                self.cursor_y = cy;
                self.update_cursor();
            }
            return;
        }

        self.erase_cursor();

        self.cursor_y += 1;
        if self.cursor_y >= self.view_height {
            self.cursor_y = self.view_height - 1;
            // scroll
            self.offset += 16;
            self.update_needed = true;
        }

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn key_up(&mut self) {
        let pos = self.offset + self.cursor_y as u64 * 16 + self.cursor_x as u64;
        if pos == 0 {
            return;
        }

        self.erase_cursor();

        if pos < 16 {
            // put cursor position at start
            self.offset = 0;
            self.cursor_x = 0;
            self.cursor_y = 0;

            self.update_cursor();
            return;
        }

        if self.cursor_y == 0 {
            // scroll
            self.offset -= 16;
            self.update_needed = true;
        } else {
            self.cursor_y -= 1;
        }

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn key_pageup(&mut self) {
        let one_page = self.view_height as u64 * 16;
        let pos = self.offset + self.cursor_y as u64 * 16;

        if pos < one_page {
            if self.cursor_y == 0 {
                if self.cursor_x == 0 {
                    return;
                }

                self.erase_cursor();
                self.cursor_x = 0;
                self.update_cursor();
                return;
            }

            self.erase_cursor();
            self.cursor_y = 0;
            self.update_cursor();
            return;
        }

        if pos < one_page * 2 {
            self.offset = 0;
            self.cursor_y = ((pos - one_page) / 16) as u16;
            self.update_needed = true;
            return;
        }

        assert!(self.offset >= one_page);
        self.offset -= one_page;
        self.update_needed = true;
    }

    fn key_pagedown(&mut self) {
        let one_page = self.view_height as u64 * 16;
        let end_offset = if self.filesize <= one_page {
            0
        } else {
            ((self.filesize + 15) / 16 * 16) - one_page
        };

        if self.offset + one_page >= end_offset {
            self.key_end();
            return;
        }

        self.offset += one_page;
        self.update_needed = true;
    }

    fn key_home(&mut self) {
        if self.offset == 0 && self.cursor_x == 0 && self.cursor_y == 0 {
            return;
        }

        if self.offset > 0 {
            self.update_needed = true;
        } else {
            self.erase_cursor();
        }

        self.offset = 0;
        self.cursor_x = 0;
        self.cursor_y = 0;

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn key_end(&mut self) {
        let one_page = self.view_height as u64 * 16;
        let end_offset = if self.filesize <= one_page {
            0
        } else {
            ((self.filesize + 15) / 16 * 16) - one_page
        };

        let cx = (self.filesize - 1 - end_offset) % 16;
        let cy = (self.filesize - 1 - end_offset) / 16;
        assert!(cy < self.view_height as u64);

        if self.offset == end_offset && self.cursor_x as u64 == cx && self.cursor_y as u64 == cy {
            return;
        }

        if self.offset != end_offset {
            self.offset = end_offset;
            self.update_needed = true;
        } else {
            self.erase_cursor();
        }

        self.cursor_x = cx as u16;
        self.cursor_y = cy as u16;

        if !self.update_needed {
            self.update_cursor();
        }
    }

    fn update_cursor(&mut self) {
        self.draw_cursor();
        self.draw_bottom_pane();
        self.stdout.flush().unwrap();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    if !stdout().is_tty() {
        eprintln!("stdout: not a tty");
        process::exit(1);
    }

    let mut hexview = HexView::new();

    let args: Vec<_> = env::args().collect();
    if args.len() <= 1 {
        let path = Path::new(&args[0]);
        let basename = path.file_name().unwrap_or(OsStr::new("rhex"));
        println!("usage: {} FILENAME", basename.to_str().unwrap());
        process::exit(1);
    }

    let filename = &args[1];
    hexview.load(filename);

    terminal::enable_raw_mode().expect("unable to put terminal in raw mode");

    let mut stdout = stdout();
    stdout
        .queue(EnterAlternateScreen)?
        .queue(Clear(ClearType::All))?
        .queue(cursor::MoveTo(0, 0))?
        .queue(cursor::Hide)?
        .queue(style::PrintStyledContent("Title".reverse()))?
        .queue(cursor::MoveTo(0, 1))?
        .flush()?;

    loop {
        hexview.draw_screen();

        let event = crossterm::event::read().expect("unable to get terminal event");
        match event {
            Event::Key(key_event) => {
                if key_event.code == KeyCode::Esc || key_event.code == KeyCode::Char('q') {
                    break;
                } else {
                    hexview.key_event(&key_event);
                }
            }
            _ => {}
        }
    }

    stdout.queue(cursor::Show)?.flush()?;

    terminal::disable_raw_mode().expect("unable to restore terminal cooked mode");
    execute!(stdout, LeaveAlternateScreen).expect("unable to restore main screen");
    println!();
    Ok(())
}

// EOB
