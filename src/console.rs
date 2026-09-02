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
            return TestResult::Fail(
                "writing characters does not advance cursor",
            );
        }

        if console.cursor_y != 0 {
            return TestResult::Fail(
                "writing characters changed cursor row",
            );
        }

        pass()
    }

    #[test]
    fn console_newline_moves_to_next_row() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abc\ndef");

        if console.cursor_x != 3 {
            return TestResult::Fail(
                "cursor column after newline is incorrect",
            );
        }

        if console.cursor_y != 1 {
            return TestResult::Fail(
                "newline did not advance row",
            );
        }

        pass()
    }

    #[test]
    fn console_carriage_return_resets_column() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abcdef\rxy");

        if console.cursor_x != 2 {
            return TestResult::Fail(
                "carriage return did not reset column",
            );
        }

        if console.cursor_y != 0 {
            return TestResult::Fail(
                "carriage return changed cursor row",
            );
        }

        pass()
    }

    #[test]
    fn console_tab_advances_to_next_tab_stop() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("a\t");

        if console.cursor_x != 4 {
            return TestResult::Fail(
                "tab did not advance to next tab stop",
            );
        }

        console.write_str("b\t");

        if console.cursor_x != 8 {
            return TestResult::Fail(
                "second tab did not advance correctly",
            );
        }

        pass()
    }

    #[test]
    fn console_backspace_moves_cursor_back() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("abc");
        console.write_str("\u{8}");

        if console.cursor_x != 2 {
            return TestResult::Fail(
                "backspace did not move cursor backwards",
            );
        }

        pass()
    }

    #[test]
    fn console_backspace_at_origin_is_safe() -> TestResult {
        let mut console = Console::new(80, 32);

        console.write_str("\u{8}");

        if console.cursor_x != 0
            || console.cursor_y != 0
        {
            return TestResult::Fail(
                "backspace at origin moved the cursor",
            );
        }

        pass()
    }

    #[test]
    fn console_wraps_at_right_edge() -> TestResult {
        let mut console = Console::new(16, 32);

        console.write_str("ab");

        if console.cursor_x != 0 {
            return TestResult::Fail(
                "console did not wrap at right edge",
            );
        }

        if console.cursor_y != 1 {
            return TestResult::Fail(
                "console did not advance row after wrapping",
            );
        }

        pass()
    }

    #[test]
    fn console_read_line_returns_none_until_line_is_ready()
        -> TestResult
    {
        let mut console = Console::new(80, 32);

        let mut buffer = [0u8; 16];

        if console.read_line(&mut buffer).is_some() {
            return TestResult::Fail(
                "read_line returned a line before Enter",
            );
        }

        pass()
    }

    #[test]
    fn console_read_line_copies_and_consumes_line()
        -> TestResult
    {
        let mut console = Console::new(80, 32);

        console.input[..5]
            .copy_from_slice(b"hello");

        console.input_len = 5;
        console.line_ready = true;

        let mut buffer = [0u8; 16];

        let length =
            match console.read_line(&mut buffer) {
                Some(length) => length,

                None => {
                    return TestResult::Fail(
                        "read_line did not return ready line",
                    );
                }
            };

        if length != 5 {
            return TestResult::Fail(
                "read_line returned incorrect length",
            );
        }

        if &buffer[..5] != b"hello" {
            return TestResult::Fail(
                "read_line returned incorrect contents",
            );
        }

        if console.input_len != 0 {
            return TestResult::Fail(
                "read_line did not consume input",
            );
        }

        if console.line_ready {
            return TestResult::Fail(
                "read_line did not clear line_ready",
            );
        }

        pass()
    }

    #[test]
    fn console_read_line_truncates_to_destination()
        -> TestResult
    {
        let mut console = Console::new(80, 32);

        console.input[..6]
            .copy_from_slice(b"abcdef");

        console.input_len = 6;
        console.line_ready = true;

        let mut buffer = [0u8; 3];

        let length =
            match console.read_line(&mut buffer) {
                Some(length) => length,

                None => {
                    return TestResult::Fail(
                        "read_line did not return ready line",
                    );
                }
            };

        if length != 3 {
            return TestResult::Fail(
                "read_line did not truncate to buffer size",
            );
        }

        if &buffer != b"abc" {
            return TestResult::Fail(
                "truncated line has incorrect contents",
            );
        }

        if console.input_len != 0
            || console.line_ready
        {
            return TestResult::Fail(
                "truncated read did not consume line",
            );
        }

        pass()
    }
}

use core::{
    fmt,
    mem::MaybeUninit,
};

use crate::{
    fb::{self, Color},
    font,
    shell,
};

const CHAR_WIDTH: usize = 8;
const CHAR_HEIGHT: usize = 16;

const DEFAULT_FOREGROUND: Color =
    Color::WHITE;

const DEFAULT_BACKGROUND: Color =
    Color::BLACK;

const INPUT_SIZE: usize = 256;

const MAX_COLUMNS: usize = 1920 / CHAR_WIDTH;
const MAX_ROWS: usize = 1080 / CHAR_HEIGHT;

// ============================================================
// Cell
// ============================================================

#[derive(Clone, Copy)]
struct Cell {
    character: u8,

    foreground: Color,

    background: Color,
}

impl Cell {
    const EMPTY: Self = Self {
        character: b' ',
        foreground: DEFAULT_FOREGROUND,
        background: DEFAULT_BACKGROUND,
    };
}

/*
The terminal cell buffer is fixed-size.

At 1920x1080:

    240 columns × 67 rows

No heap allocation.
*/

static mut CELLS:
    [Cell; MAX_COLUMNS * MAX_ROWS] =
    [Cell::EMPTY; MAX_COLUMNS * MAX_ROWS];

// ============================================================
// Console
// ============================================================

pub struct Console {
    font: font::Font,

    cursor_x: usize,
    cursor_y: usize,

    columns: usize,
    rows: usize,

    foreground: Color,
    background: Color,

    cells: *mut Cell,

    input: [u8; INPUT_SIZE],

    input_len: usize,
    line_ready: bool,
}

// ============================================================
// Construction
// ============================================================

impl Console {
    pub fn new(
        width: usize,
        height: usize,
    ) -> Self {
        let columns =
            (width / CHAR_WIDTH)
                .min(MAX_COLUMNS);

        let rows =
            (height / CHAR_HEIGHT)
                .min(MAX_ROWS);

        let cells =
            core::ptr::addr_of_mut!(CELLS)
                as *mut Cell;

        unsafe {
            let cells =
                core::slice::from_raw_parts_mut(
                    cells,
                    MAX_COLUMNS * MAX_ROWS,
                );

            cells.fill(Cell::EMPTY);
        }

        Self {
            font: font::spleen(),

            cursor_x: 0,
            cursor_y: 0,

            columns,
            rows,

            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,

            cells,

            input: [0; INPUT_SIZE],

            input_len: 0,
            line_ready: false,
        }
    }

    // ========================================================
    // Cell access
    // ========================================================

    #[inline(always)]
    fn cell_index(
        row: usize,
        column: usize,
    ) -> usize {
        row * MAX_COLUMNS + column
    }

    #[inline(always)]
    fn get_cell(
        &self,
        row: usize,
        column: usize,
    ) -> Cell {
        unsafe {
            *self.cells.add(
                Self::cell_index(
                    row,
                    column,
                ),
            )
        }
    }

    #[inline(always)]
    fn set_cell(
        &mut self,
        row: usize,
        column: usize,
        cell: Cell,
    ) {
        unsafe {
            self.cells.add(
                Self::cell_index(
                    row,
                    column,
                )
            ).write(cell);
        }
    }

    fn clear_row(
        &mut self,
        row: usize,
    ) {
        if row >= self.rows {
            return;
        }

        unsafe {
            let start =
                self.cells.add(
                    Self::cell_index(
                        row,
                        0,
                    ),
                );

            let row =
                core::slice::from_raw_parts_mut(
                    start,
                    MAX_COLUMNS,
                );

            row.fill(Cell::EMPTY);
        }
    }

    // ========================================================
    // Clear
    // ========================================================

    pub fn clear(&mut self) {
        unsafe {
            let cells =
                core::slice::from_raw_parts_mut(
                    self.cells,
                    MAX_COLUMNS * MAX_ROWS,
                );

            cells.fill(Cell::EMPTY);
        }

        self.cursor_x = 0;
        self.cursor_y = 0;

        self.input_len = 0;
        self.line_ready = false;

        fb::clear(self.background);
    }

    // ========================================================
    // Colors
    // ========================================================

    #[expect(unused)]
    pub fn set_foreground(
        &mut self,
        color: Color,
    ) {
        self.foreground = color;
    }

    #[expect(unused)]
    pub fn set_background(
        &mut self,
        color: Color,
    ) {
        self.background = color;
    }

    #[expect(unused)]
    pub fn get_background(
        &mut self,
    ) -> Color {
        self.background
    }

    #[expect(unused)]
    pub fn get_foreground(
        &mut self,
    ) -> Color {
        self.foreground
    }

    // ========================================================
    // Render cell
    // ========================================================

    /*
    Draw exactly one terminal cell.

    This is the hot path for typing.
    */

    fn render_cell(
        &self,
        row: usize,
        column: usize,
    ) {
        if row >= self.rows
            || column >= self.columns
        {
            return;
        }

        let cell =
            self.get_cell(
                row,
                column,
            );

        let x =
            column * CHAR_WIDTH;

        let y =
            row * CHAR_HEIGHT;

        /*
        Clear the cell first because the font only paints
        foreground pixels.
        */

        fb::draw_rect(
            x,
            y,
            CHAR_WIDTH,
            CHAR_HEIGHT,
            cell.background,
        );

        if cell.character != b' ' {
            self.font.draw_char(
                x,
                y,
                cell.character,
                cell.foreground,
            );
        }
    }

    // ========================================================
    // Render row
    // ========================================================

    fn render_row(
        &self,
        row: usize,
    ) {
        if row >= self.rows {
            return;
        }

        for column in 0..self.columns {
            self.render_cell(
                row,
                column,
            );
        }
    }

    // ========================================================
    // Full render
    // ========================================================

    /*
    Full redraw of the terminal.

    This is NOT used when typing normal characters.
    */

    pub fn render(&self) {
        fb::clear(
            self.background,
        );

        for row in 0..self.rows {
            self.render_row(row);
        }
    }

    // ========================================================
    // Scroll
    // ========================================================

    fn scroll(&mut self) {
        if self.rows == 0 {
            return;
        }

        /*
        First move the framebuffer pixels upward by exactly
        one character height.

            16 pixels
        */
        fb::scroll_up(
            CHAR_HEIGHT,
            self.background,
        );

        /*
        Now move the terminal's logical cells upward by one row.

            row 1 -> row 0
            row 2 -> row 1
            row 3 -> row 2
            ...
            row N -> row N-1

        This keeps the cell buffer synchronized with the
        framebuffer.
        */

        unsafe {
            let cells =
                core::slice::from_raw_parts_mut(
                    self.cells,
                    MAX_COLUMNS * MAX_ROWS,
                );

            for row in 1..self.rows {
                let src =
                    row * MAX_COLUMNS;

                let dst =
                    (row - 1) * MAX_COLUMNS;

                cells.copy_within(
                    src..src + self.columns,
                    dst,
                );
            }
        }

        /*
        Clear the newly exposed bottom terminal row.
        */

        self.clear_row(
            self.rows - 1,
        );

        /*
        Cursor is now at the beginning of the new bottom row.
        */

        self.cursor_x = 0;

        self.cursor_y =
            self.rows - 1;
    }

    // ========================================================
    // Newline
    // ========================================================

    fn newline(&mut self) {
        self.cursor_x = 0;

        if self.rows == 0 {
            return;
        }

        if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        } else {
            self.scroll();
        }
    }

    // ========================================================
    // Put character
    // ========================================================

    fn put_char(
        &mut self,
        character: u8,
    ) {
        match character {
            b'\n' => {
                self.newline();
            }

            b'\r' => {
                self.cursor_x = 0;
            }

            b'\t' => {
                const TAB_SIZE: usize = 4;

                let next_tab =
                    ((self.cursor_x / TAB_SIZE) + 1)
                        * TAB_SIZE;

                if next_tab >= self.columns {
                    self.newline();
                } else {
                    self.cursor_x =
                        next_tab;
                }
            }

            8 => {
                self.backspace();
            }

            0x20..=0x7e => {
                self.write_cell(
                    character,
                );
            }

            _ => {}
        }
    }

    // ========================================================
    // Write cell
    // ========================================================

    fn write_cell(
        &mut self,
        character: u8,
    ) {
        if self.columns == 0
            || self.rows == 0
        {
            return;
        }

        if self.cursor_x >= self.columns {
            self.newline();
        }

        let row =
            self.cursor_y;

        let column =
            self.cursor_x;

        let cell = Cell {
            character,

            foreground:
                self.foreground,

            background:
                self.background,
        };

        self.set_cell(
            row,
            column,
            cell,
        );

        /*
        Only this 8x16 cell gets rendered.
        */

        self.render_cell(
            row,
            column,
        );

        self.cursor_x += 1;

        /*
        Wrapping at the right edge.
        */

        if self.cursor_x >= self.columns {
            self.newline();
        }
    }

    // ========================================================
    // Backspace
    // ========================================================

    fn backspace(&mut self) {
        if self.cursor_x == 0 {
            if self.cursor_y == 0 {
                return;
            }

            self.cursor_y -= 1;

            self.cursor_x =
                self.columns
                    .saturating_sub(1);
        } else {
            self.cursor_x -= 1;
        }

        self.set_cell(
            self.cursor_y,
            self.cursor_x,
            Cell::EMPTY,
        );

        /*
        Only redraw the erased character cell.
        */

        self.render_cell(
            self.cursor_y,
            self.cursor_x,
        );
    }

    // ========================================================
    // Write string
    // ========================================================

    pub fn write_str(
        &mut self,
        text: &str,
    ) {
        for byte in text.bytes() {
            self.put_char(byte);
        }
    }

    // ========================================================
    // Keyboard
    // ========================================================
    pub fn receive_key(&mut self, key: char) {
        match key {
            '\n' | '\r' => {
                self.put_char(b'\n');

                self.line_ready = true;
            }

            '\u{8}' | '\u{7f}' => {
                if self.input_len > 0 && self.cursor_x > 2 {
                    self.input_len -= 1;
                    self.put_char(8);
                }
            }

            character
                if character.is_ascii()
                    && !character.is_ascii_control() =>
            {
                if self.input_len < INPUT_SIZE {
                    self.input[self.input_len] = character as u8;
                    self.input_len += 1;

                    self.put_char(character as u8);
                }
            }

            _ => {}
        }
    }

    // ========================================================
    // Read line
    // ========================================================
    pub fn read_line(
        &mut self,
        buffer: &mut [u8],
    ) -> Option<usize> {
        if !self.line_ready {
            return None;
        }

        let length =
            self.input_len
                .min(buffer.len());

        buffer[..length]
            .copy_from_slice(
                &self.input[..length],
            );

        self.input_len = 0;
        self.line_ready = false;

        Some(length)
    }
}

// ============================================================
// fmt::Write
// ============================================================

impl fmt::Write for Console {
    fn write_str(
        &mut self,
        text: &str,
    ) -> fmt::Result {
        self.write_str(text);

        Ok(())
    }
}

// ============================================================
// Global console
// ============================================================

static mut CONSOLE:
    MaybeUninit<Console> =
    MaybeUninit::uninit();

static mut CONSOLE_INITIALIZED:
    bool = false;

// ============================================================
// Initialization
// ============================================================

pub fn init() {
    let width =
        fb::width();

    let height =
        fb::height();

    let console =
        Console::new(
            width,
            height,
        );

    unsafe {
        core::ptr::addr_of_mut!(
            CONSOLE
        )
        .write(
            MaybeUninit::new(
                console,
            ),
        );

        core::ptr::addr_of_mut!(
            CONSOLE_INITIALIZED
        )
        .write(true);
    }

    with_console(|console| {
        console.clear();
    });

    with_console(|console| {
        console.put_char(b'>');
        console.put_char(b' ');
    });

    fb::present();
}

// ============================================================
// with_console
// ============================================================

pub fn with_console<F, R>(
    f: F,
) -> R
where
    F: FnOnce(&mut Console) -> R,
{
    unsafe {
        if !CONSOLE_INITIALIZED {
            panic!(
                "console used before console::init()"
            );
        }

        let console =
            core::ptr::addr_of_mut!(
                CONSOLE
            )
            .cast::<Console>()
            .as_mut()
            .unwrap_unchecked();

        f(console)
    }
}

// ============================================================
// Formatted output
// ============================================================

pub fn write_fmt(
    args: fmt::Arguments<'_>,
) {
    use core::fmt::Write;

    /*
    Don't call present() for every character.

    Formatting "hello world" changes many cells, but we only
    need one final presentation.
    */

    with_console(|console| {
        let _ =
            console.write_fmt(args);
    });

    fb::present();
}

// ============================================================
// Keyboard
// ============================================================
pub fn receive_key(key: char) {
    if key == '\n' || key == '\r' {
        let mut command_buffer = [0u8; INPUT_SIZE];

        let length = with_console(|console| {
            console.receive_key(key);

            console.read_line(&mut command_buffer)
        });

        if let Some(length) = length {
            if let Ok(command) =
                core::str::from_utf8(&command_buffer[..length])
            {
                shell::execute(command);
            }

            with_console(|console| {
                console.put_char(b'>');
                console.put_char(b' ');
            });
        }

        fb::present();

        return;
    }

    with_console(|console| {
        console.receive_key(key);
    });

    fb::present();
}

// ============================================================
// Read line
// ============================================================

pub fn read_line(
    buffer: &mut [u8],
) -> Option<usize> {
    with_console(|console| {
        console.read_line(buffer)
    })
}

// ============================================================
// Clear
// ============================================================

pub fn clear() {
    with_console(|console| {
        console.clear();
    });

    fb::present();
}

// ============================================================
// Printing macros
// ============================================================

#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => {
        $crate::console::write_fmt(
            core::format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! console_println {
    () => {
        $crate::console::write_fmt(
            core::format_args!("\n")
        )
    };

    ($($arg:tt)*) => {
        $crate::console::write_fmt(
            core::format_args!(
                "{}\n",
                core::format_args!($($arg)*)
            )
        )
    };
}
