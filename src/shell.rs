use x86_64::{VirtAddr, structures::paging::{PageTable, Translate}};

use crate::{acpi, console::{self, Console, clear, with_console}, console_print, console_println, console_println_color, fb::{self, Color}, kmem::{self, active_level_4_table, hhdm_offset, translate_addr}, test::exit_qemu};
use crate::test::{test, TestResult};

pub fn execute(line: &str) {
    let mut parts = line.split_whitespace();

    let command = match parts.next() {
        Some(command) => command,
        None => return,
    };

    match command {
        "help" => help(),
        "clear" => clearterm(),
        "echo" => echo(parts),
        "info" => info(),
        "panic" => panic(),
        "bp" => bp(),
        "sven" => sven(), //NOTE: Do not add this to help
        "reboot" => reboot(),
        "shutdown" => shutdown(),
        "exit" => shutdown(),
        "setbg" => setbg(parts),
        "setfg" => setfg(parts),
        "mem-analyze" => mem_analyze(),
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
    console_println!("  exit       - Shuts the computer down.");
    console_println!("  setbg      - Sets the background color.");
    console_println!("  setfg      - Sets the foreground color.");
}

fn clearterm() {
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
    console_println!("Framebuffer: {}x{}", fb::width(), fb::height());

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

fn mem_analyze(){
    let phys_mem_offset = VirtAddr::new(hhdm_offset());
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        if !entry.is_unused() {
            console_println_color!(Color::GREEN, "L4 Entry {}: {:?}", i, entry);

            // get the physical address from the entry and convert it
            let phys = entry.frame().unwrap().start_address();
            let virt = phys.as_u64() + hhdm_offset();
            let ptr = VirtAddr::new(virt).as_mut_ptr();
            let l3_table: &PageTable = unsafe { &*ptr };

            // print non-empty entries of the level 3 table
            for (i, entry) in l3_table.iter().enumerate() {
                if !entry.is_unused() {
                    console_println_color!(Color::GREEN, "  L3 Entry {}: {:?}", i, entry);
                }
            }
        }
    }

}

//TESTS

fn test_set_color(
    color_name: &str,
    expected: Color,
    set_color: fn(core::str::SplitWhitespace<'_>),
    get_color: fn(&mut Console) -> Color,
    error_message: &'static str,
) -> TestResult {
    set_color(color_name.split_whitespace());

    with_console(|console| {
        if get_color(console) == expected {
            TestResult::Pass
        } else {
            TestResult::Fail(error_message)
        }
    })
}

///DO NOT USE GLOBALLY.
///Only to be used if you know EXACTLY what this does.
///Youve been warned.
macro_rules! color_test {
    ($test_name:ident, $setter:ident, $getter:path, $name:expr, $color:expr, $error:expr) => {
        #[test]
        fn $test_name() -> TestResult {
            test_set_color(
                $name,
                $color,
                $setter,
                $getter,
                $error,
            )
        }
    };
}

color_test!(
    test_setbg_red,
    setbg,
    Console::get_background,
    "red",
    Color::RED,
    "set red failed"
);

color_test!(
    test_setbg_black,
    setbg,
    Console::get_background,
    "black",
    Color::BLACK,
    "set black failed"
);

color_test!(
    test_setbg_green,
    setbg,
    Console::get_background,
    "green",
    Color::GREEN,
    "set green failed"
);

color_test!(
    test_setbg_blue,
    setbg,
    Console::get_background,
    "blue",
    Color::BLUE,
    "set blue failed"
);

color_test!(
    test_setbg_white,
    setbg,
    Console::get_background,
    "white",
    Color::WHITE,
    "set white failed"
);

color_test!(
    test_setfg_red,
    setfg,
    Console::get_foreground,
    "red",
    Color::RED,
    "set red failed"
);

color_test!(
    test_setfg_black,
    setfg,
    Console::get_foreground,
    "black",
    Color::BLACK,
    "set black failed"
);

color_test!(
    test_setfg_green,
    setfg,
    Console::get_foreground,
    "green",
    Color::GREEN,
    "set green failed"
);

color_test!(
    test_setfg_blue,
    setfg,
    Console::get_foreground,
    "blue",
    Color::BLUE,
    "set blue failed"
);

color_test!(
    test_setfg_white,
    setfg,
    Console::get_foreground,
    "white",
    Color::WHITE,
    "set white failed"
);
