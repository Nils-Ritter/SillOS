use crate::{print, println};

pub fn main(_args: &[&str]) -> i32 {
    println!("SillOS - DEV BUILD");
    println!("Avaliable commands:");
    println!("\thelp - Prints a list of avaliable commands.");
    print!("\techo - Takes in text as argument and repeats it back to you.");
    return 0;
}
