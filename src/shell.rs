use crate::{acpi, console::{self, Console, with_console}, console_print, console_println, fb::Color, test::exit_qemu};

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
        "panic" => panic(),
        "bp" => bp(),
        "sven" => sven(), //NOTE: Do not add this to help
        "reboot" => reboot(),
        "shutdown" => shutdown(),
        "setbg" => setbg(parts),
        "setfg" => setfg(parts),


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
    console_println!("  panic      - Intentionally throws a kernel panic");
    console_println!("  bp         - Sets and steps over a breakpoint");
    console_println!("  reboot     - Reboots the machine.");
    console_println!("  shutdown   - Shuts the computer down.");
    console_println!("  setbg      - Sets the background color.");
    console_println!("  setfg      - Sets the foreground color.");
}

fn clear() {
    console::clear();
}

fn echo(args: core::str::SplitWhitespace<'_>) {
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

fn panic(){
    panic!("Intentional debug panic");
}

fn bp(){
    x86_64::instructions::interrupts::int3();
}

fn sven(){
    console_println!("This command is dedicated to my friend bunny, sven!");
    console_println!("Say bye bye to your pc :)");
    acpi::reboot();
}

fn reboot(){
    acpi::reboot();
}

fn shutdown(){
    console_println!("There currently is no support for acpi shutdown.");
    console_println!("However, qemu will close normally.");
    exit_qemu(true);
}

fn setbg(mut args: core::str::SplitWhitespace<'_>) {
    let Some(color_name) = args.next() else {
        console_println!("Incorrect usage!");
        console_println!("Usage: setbg <color>");
        return;
    };

    let Some(color) = Color::from_name(color_name) else {
        console_println!("Unknown color: {}", color_name);
        return;
    };

    with_console(|console| {
        Console::set_background(console, color);
    });

    with_console(|console| {
        Console::clear(console);
    });
}

fn setfg(mut args: core::str::SplitWhitespace<'_>) {
    let Some(color_name) = args.next() else {
        console_println!("Incorrect usage!");
        console_println!("Usage: setfg <color>");
        return;
    };

    let Some(color) = Color::from_name(color_name) else {
        console_println!("Unknown color: {}", color_name);
        return;
    };

    with_console(|console| {
        Console::set_foreground(console, color);
    });

    with_console(|console| {
        Console::clear(console);
    });
}
