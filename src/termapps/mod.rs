pub mod echo;
pub mod help;
pub mod clear;

pub type AppMain = fn(&[&str]) -> i32;

pub static APPS: &[(&str, AppMain)] = &[
    ("help", help::main),
    ("echo", echo::main),
    ("clear", clear::main),
];
