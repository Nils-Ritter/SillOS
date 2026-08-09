use crate::{print, println, vga_buffer::WRITER};

pub fn main(args: &[&str]) -> i32 {
    let mut writer = WRITER.lock();
    writer.clear_screen();
    return 0;
}
