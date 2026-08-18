use crate::fb::Color;

const PSF2_MAGIC: [u8; 4] = [0x72, 0xb5, 0x4a, 0x86];

const FONT_WIDTH: usize = 16;
const FONT_HEIGHT: usize = 32;

const SCALED_WIDTH: usize = FONT_WIDTH / 2;
const SCALED_HEIGHT: usize = FONT_HEIGHT / 2;

static FONT_DATA: &[u8] = include_bytes!("assets/fonts/spleen-2.2.0/spleen-16x32.psfu");

// ============================================================
// PSF2 helpers
// ============================================================

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

// ============================================================
// Font
// ============================================================

pub struct Font {
    data: &'static [u8],

    header_size: usize,
    glyph_count: usize,
    glyph_size: usize,
}

impl Font {
    pub fn new(data: &'static [u8]) -> Self {
        assert!(data.len() >= 32, "Font file is too small");

        assert_eq!(&data[0..4], &PSF2_MAGIC, "Not a PSF2 font");

        let version = read_u32(data, 4);

        assert_eq!(version, 0, "Unsupported PSF2 version");

        let header_size = read_u32(data, 8) as usize;

        let glyph_count = read_u32(data, 16) as usize;

        let glyph_size = read_u32(data, 20) as usize;

        let width = read_u32(data, 28) as usize;

        let height = read_u32(data, 24) as usize;

        assert_eq!(width, FONT_WIDTH, "Expected a 16 pixel wide font");

        assert_eq!(height, FONT_HEIGHT, "Expected a 32 pixel high font");

        Self {
            data,
            header_size,
            glyph_count,
            glyph_size,
        }
    }

    fn glyph(&self, index: usize) -> &[u8] {
        if index >= self.glyph_count {
            return &[];
        }

        let offset = self.header_size + index * self.glyph_size;

        &self.data[offset..offset + self.glyph_size]
    }

    // ========================================================
    // Draw a single 8x16 character
    // ========================================================

    pub fn draw_char(&self, x: usize, y: usize, character: u8, color: Color) {
        let glyph = self.glyph(character as usize);

        if glyph.is_empty() {
            return;
        }

        //
        // Original font:
        //
        //     16 x 32
        //
        // We render every second pixel:
        //
        //      8 x 16
        //
        let bytes_per_row = 2;

        for row in 0..SCALED_HEIGHT {
            let source_y = row * 2;

            for col in 0..SCALED_WIDTH {
                let source_x = col * 2;

                let byte = glyph[source_y * bytes_per_row + source_x / 8];

                let bit = 7 - (source_x % 8);

                if byte & (1 << bit) != 0 {
                    crate::fb::put_pixel(x + col, y + row, color);
                }
            }
        }
    }

    // ========================================================
    // Draw text
    // ========================================================

    pub fn draw_text(&self, x: usize, y: usize, text: &str, color: Color) {
        let mut cursor_x = x;
        let mut cursor_y = y;

        for character in text.bytes() {
            match character {
                b'\n' => {
                    cursor_x = x;
                    cursor_y += SCALED_HEIGHT;
                }

                b'\r' => {
                    cursor_x = x;
                }

                b'\t' => {
                    cursor_x += SCALED_WIDTH * 4;
                }

                0x20..=0x7e => {
                    self.draw_char(cursor_x, cursor_y, character, color);

                    cursor_x += SCALED_WIDTH;
                }

                _ => {
                    // Ignore unsupported characters.
                }
            }
        }
    }
}

// ============================================================
// Spleen font
// ============================================================

pub fn spleen() -> Font {
    Font::new(FONT_DATA)
}
