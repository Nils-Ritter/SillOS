#[cfg(feature = "test")]
mod tests {
    use crate::test::{test, TestResult};

    use super::Console;

    fn pass() -> TestResult {
        TestResult::Pass
    }

    #[test]
    fn console_initializes_cursor_and_input_state() -> TestResult {
        let console = Console::new(800, 480);

        if console.cursor_x != 0 {
            return TestResult::Fail("cursor_x is not initialized to zero");
        }

        if console.cursor_y != 0 {
            return TestResult::Fail("cursor_y is not initialized to zero");
        }

        if console.columns != 100 {
            return TestResult::Fail("console column count is incorrect");
        }

        if console.rows != 30 {
            return TestResult::Fail("console row count is incorrect");
        }

        if console.input_len != 0 {
            return TestResult::Fail("input buffer is not empty");
        }

        if console.line_ready {
            return TestResult::Fail("line_ready is initially set");
        }

        pass()
    }

    #[test]
    fn console_writes_characters() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abc");

        if console.cursor_x != 3 {
            return TestResult::Fail("writing characters does not advance cursor");
        }

        if console.cursor_y != 0 {
            return TestResult::Fail("writing characters changed cursor row");
        }

        pass()
    }

    #[test]
    fn console_newline_moves_to_next_row() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abc\ndef");

        if console.cursor_x != 3 {
            return TestResult::Fail("cursor column after newline is incorrect");
        }

        if console.cursor_y != 1 {
            return TestResult::Fail("newline did not advance cursor row");
        }

        pass()
    }

    #[test]
    fn console_carriage_return_resets_column() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abcdef\rxy");

        if console.cursor_x != 2 {
            return TestResult::Fail("carriage return did not reset cursor column");
        }

        if console.cursor_y != 0 {
            return TestResult::Fail("carriage return changed cursor row");
        }

        pass()
    }

    #[test]
    fn console_tab_advances_to_next_tab_stop() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("a\t");

        if console.cursor_x != 4 {
            return TestResult::Fail("tab did not advance to the next tab stop");
        }

        console.write_str("b\t");

        if console.cursor_x != 8 {
            return TestResult::Fail("second tab did not advance correctly");
        }

        pass()
    }

    #[test]
    fn console_backspace_moves_cursor_back() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abc");
        console.write_str("\u{8}");

        if console.cursor_x != 2 {
            return TestResult::Fail("backspace did not move cursor backwards");
        }

        pass()
    }

    #[test]
    fn console_backspace_at_origin_is_safe() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("\u{8}");

        if console.cursor_x != 0 || console.cursor_y != 0 {
            return TestResult::Fail("backspace at origin moved the cursor");
        }

        pass()
    }

    #[test]
    fn console_wraps_at_right_edge() -> TestResult {
        let mut console = Console::new(16, 32);

        console.write_str("ab");

        if console.cursor_x != 0 {
            return TestResult::Fail("console did not wrap at the right edge");
        }

        if console.cursor_y != 1 {
            return TestResult::Fail("console did not advance row after wrapping");
        }

        pass()
    }

    #[test]
    fn console_read_line_returns_none_until_line_is_ready() -> TestResult {
        let mut console = Console::new(80, 32);
        let mut buffer = [0u8; 16];

        if console.read_line(&mut buffer).is_some() {
            return TestResult::Fail("read_line returned a line before Enter");
        }

        pass()
    }

    #[test]
    fn console_read_line_copies_and_consumes_line() -> TestResult {
        let mut console = Console::new(80, 32);

        console.input[..5].copy_from_slice(b"hello");
        console.input_len = 5;
        console.line_ready = true;

        let mut buffer = [0u8; 16];

        let length = match console.read_line(&mut buffer) {
            Some(length) => length,
            None => return TestResult::Fail("read_line did not return ready line"),
        };

        if length != 5 {
            return TestResult::Fail("read_line returned incorrect length");
        }

        if &buffer[..5] != b"hello" {
            return TestResult::Fail("read_line returned incorrect contents");
        }

        if console.input_len != 0 {
            return TestResult::Fail("read_line did not consume input");
        }

        if console.line_ready {
            return TestResult::Fail("read_line did not clear line_ready");
        }

        pass()
    }

    #[test]
    fn console_read_line_truncates_to_destination() -> TestResult {
        let mut console = Console::new(80, 32);

        console.input[..6].copy_from_slice(b"abcdef");
        console.input_len = 6;
        console.line_ready = true;

        let mut buffer = [0u8; 3];

        let length = match console.read_line(&mut buffer) {
            Some(length) => length,
            None => return TestResult::Fail("read_line did not return ready line"),
        };

        if length != 3 {
            return TestResult::Fail("read_line did not truncate to buffer size");
        }

        if &buffer != b"abc" {
            return TestResult::Fail("truncated line has incorrect contents");
        }

        if console.input_len != 0 || console.line_ready {
            return TestResult::Fail("truncated read did not consume the line");
        }

        pass()
    }
}

use core::{ fmt, mem::MaybeUninit, };
use crate::{
    fb::{self, Color}, font, shell,
};

const CHAR_WIDTH: usize = 8;
const CHAR_HEIGHT: usize = 16;

const DEFAULT_FOREGROUND: Color = Color::WHITE;
const DEFAULT_BACKGROUND: Color = Color::BLACK;

const INPUT_SIZE: usize = 256;

pub struct Console {
    font: font::Font,

    cursor_x: usize,
    cursor_y: usize,

    columns: usize,
    rows: usize,

    foreground: Color,
    background: Color,

    input: [u8; INPUT_SIZE],
    input_len: usize,
    line_ready: bool,
}

impl Console {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            font: font::spleen(),

            cursor_x: 0,
            cursor_y: 0,

            columns: width / CHAR_WIDTH,
            rows: height / CHAR_HEIGHT,

            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,

            input: [0; INPUT_SIZE],
            input_len: 0,
            line_ready: false,
        }
    }

    pub fn clear(&mut self) {
        fb::clear(self.background);

        self.cursor_x = 0;
        self.cursor_y = 0;

        self.input_len = 0;
        self.line_ready = false;
    }

    #[expect(unused)]
    pub fn set_foreground(&mut self, color: Color) {
        self.foreground = color;
    }

    #[expect(unused)]
    pub fn set_background(&mut self, color: Color) {
        self.background = color;
    }

    fn newline(&mut self) {
        self.cursor_x = 0;

        if self.cursor_y + 1 >= self.rows {
            self.scroll();
        } else {
            self.cursor_y += 1;
        }
    }

    fn scroll(&mut self) {
        /*
         * The framebuffer module currently doesn't expose a blit/copy
         * operation, so redraw the console background here.
         *
         * This keeps the console independent from framebuffer internals.
         *
         * Once fb.rs exposes a backbuffer copy operation, this can be
         * replaced with a real pixel scroll.
         */
        fb::clear(self.background);

        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    fn put_char(&mut self, character: u8) {
        match character {
            b'\n' => {
                self.newline();
            }

            b'\r' => {
                self.cursor_x = 0;
            }

            b'\t' => {
                const TAB_SIZE: usize = 4;

                let next_tab = ((self.cursor_x / TAB_SIZE) + 1) * TAB_SIZE;

                if next_tab >= self.columns {
                    self.newline();
                } else {
                    self.cursor_x = next_tab;
                }
            }

            8 => {
                self.backspace();
            }

            0x20..=0x7e => {
                self.draw_character(character);
            }

            _ => {}
        }
    }

    fn draw_character(&mut self, character: u8) {
        if self.columns == 0 || self.rows == 0 {
            return;
        }

        if self.cursor_x >= self.columns {
            self.newline();
        }

        if self.cursor_y >= self.rows {
            self.scroll();
        }

        let x = self.cursor_x * CHAR_WIDTH;
        let y = self.cursor_y * CHAR_HEIGHT;

        /*
         * Clear the entire character cell first.
         *
         * font::Font::draw_char only paints foreground pixels, so without
         * this, overwriting a character would leave old pixels behind.
         */
        fb::draw_rect(
            x,
            y,
            CHAR_WIDTH,
            CHAR_HEIGHT,
            self.background,
        );

        self.font.draw_char(
            x,
            y,
            character,
            self.foreground,
        );

        self.cursor_x += 1;

        if self.cursor_x >= self.columns {
            self.newline();
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x == 0 {
            if self.cursor_y == 0 {
                return;
            }

            self.cursor_y -= 1;
            self.cursor_x = self.columns.saturating_sub(1);
        } else {
            self.cursor_x -= 1;
        }

        let x = self.cursor_x * CHAR_WIDTH;
        let y = self.cursor_y * CHAR_HEIGHT;

        fb::draw_rect(
            x,
            y,
            CHAR_WIDTH,
            CHAR_HEIGHT,
            self.background,
        );
    }

    pub fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            self.put_char(byte);
        }
    }

    /*
     * Called by the keyboard interrupt path.
     */
    pub fn receive_key(&mut self, key: char) {
        match key {
            '\n' | '\r' => {
                let mut cmdbuf = [0u8; 256];
                self.put_char(b'\n');
                if let Some(length) = read_line(&mut cmdbuf) {
                    if let Ok(command) = core::str::from_utf8(&cmdbuf[..length]) {
                        shell::execute(command);
                    }
                }
                self.put_char(b'>');
                self.put_char(b' ');
                self.line_ready = true;
            }

            '\u{8}' | '\u{7f}' => {
                if self.input_len > 0 && self.cursor_x > 2 {
                    self.input_len -= 1;
                    self.put_char(8);
                }
            }

            character if character.is_ascii() && !character.is_ascii_control() => {
                if self.input_len < INPUT_SIZE {
                    self.input[self.input_len] = character as u8;
                    self.input_len += 1;

                    self.put_char(character as u8);
                }
            }

            _ => {}
        }

        fb::present();
    }

    /*
     * Copies the current input line into `buffer`.
     *
     * Returns Some(length) when Enter was pressed.
     * Returns None when a complete line is not available yet.
     */
    pub fn read_line(&mut self, buffer: &mut [u8]) -> Option<usize> {
        if !self.line_ready {
            return None;
        }

        let length = self.input_len.min(buffer.len());

        buffer[..length].copy_from_slice(&self.input[..length]);

        self.input_len = 0;
        self.line_ready = false;

        Some(length)
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write_str(text);
        Ok(())
    }
}


/*
 * Global kernel console
 *
 * SillOS is currently single-core/simple enough that this can be used
 * without introducing a dependency on a synchronization primitive.
 *
 * The console should not be accessed simultaneously from multiple
 * interrupt contexts.
 */

static mut CONSOLE: MaybeUninit<Console> = MaybeUninit::uninit();
static mut CONSOLE_INITIALIZED: bool = false;

pub fn init() {
    let width = fb::width();
    let height = fb::height();

    let console = Console::new(width, height);

    unsafe {
        core::ptr::addr_of_mut!(CONSOLE)
            .write(MaybeUninit::new(console));

        core::ptr::addr_of_mut!(CONSOLE_INITIALIZED)
            .write(true);
    }
    with_console(|console| {
        console.clear();
    });
    receive_key('\n');
    fb::present();
}

fn with_console<F, R>(f: F) -> R
where
    F: FnOnce(&mut Console) -> R,
{
    unsafe {
        if !CONSOLE_INITIALIZED {
            panic!("console used before console::init()");
        }

        let console =
            core::ptr::addr_of_mut!(CONSOLE)
                .cast::<Console>()
                .as_mut()
                .unwrap_unchecked();

        f(console)
    }
}

pub fn write_fmt(args: fmt::Arguments<'_>) {
    use core::fmt::Write;

    with_console(|console| {
        let _ = console.write_fmt(args);
    });
}

pub fn receive_key(key: char) {
    with_console(|console| {
        console.receive_key(key);
    });
}

#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => {
        $crate::console::write_fmt(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! console_println {
    () => {
        $crate::console::write_fmt(core::format_args!("\n"))
    };

    ($($arg:tt)*) => {
        $crate::console::write_fmt(
            core::format_args!("{}\n", core::format_args!($($arg)*))
        )
    };
}

pub fn read_line(buffer: &mut [u8]) -> Option<usize> {
    with_console(|console| console.read_line(buffer))
}

pub fn clear(){
    with_console(|console| {
        console.clear();
    });
    fb::present();
}
