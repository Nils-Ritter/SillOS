use crate::{console, console_print, console_println};

pub fn execute(line: &str) {
    let mut parts = line.split_whitespace();

    let command = match parts.next() {
        Some(command) => command,
        None => return,
    };

    match command {
        "help" => help(),
        "clear" => clear(),
        "echo" => echo(parts),
        "info" => info(),

        _ => {
            console_println!("Unknown command: {}", command);
            console_println!("Type 'help' for a list of commands.");
        }
    }
}

fn help() {
    console_println!("Available commands:");
    console_println!("  help       - Show this help");
    console_println!("  clear      - Clear the screen");
    console_println!("  echo       - Print text");
    console_println!("  info       - Show system information");
}

fn clear() {
    console::clear();
}

fn echo(mut args: core::str::SplitWhitespace<'_>) {
    let mut first = true;

    for arg in args {
        if !first {
            console_print!(" ");
        }

        console_print!("{}", arg);
        first = false;
    }

    console_println!();
}

fn info() {
    console_println!("SillOS");
    console_println!("Architecture: x86_64");
    console_println!("Bootloader: Limine");
}
