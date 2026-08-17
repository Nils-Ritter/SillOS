use crate::println;

pub fn main(_args: &[&str]) -> i32 {
    println!("SillOS - DEV BUILD");
    println!("Avaliable commands:");
    println!("\thelp - Prints a list of avaliable commands.");
    println!("\techo - Takes in text as argument and repeats it back to you.");
    println!("\tbp - Sets a breakpoint and steps over it.");
    println!("\tpanic - Intentionally panics the kernel.");
    return 0;
}
