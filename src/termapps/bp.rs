pub fn main(_args: &[&str]) -> i32 {
    x86_64::instructions::interrupts::int3();
    return 0;
}
