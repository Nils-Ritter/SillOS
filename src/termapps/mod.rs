pub mod echo;
pub mod help;
pub mod clear;
pub mod panic;
pub mod bp;

pub type AppMain = fn(&[&str]) -> i32;

pub static APPS: &[(&str, AppMain)] = &[
    ("help", help::main),
    ("echo", echo::main),
    ("clear", clear::main),
    ("panic", panic::main),
    ("bp", bp::main),
];
