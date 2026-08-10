use crate::vga_buffer::WRITER;

pub fn main(_args: &[&str]) -> i32 {
    let mut writer = WRITER.lock();
    writer.clear_screen();
    return 0;
}
