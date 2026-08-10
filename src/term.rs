use ext_alloc::{string::{String, ToString}, vec::Vec};
use spin::Mutex;

use crate::{print, termapps};

static COMMAND_LINE: Mutex<String> = Mutex::new(String::new());

pub fn append_cmd_char(c: char){
    let mut cmd = COMMAND_LINE.lock();
    cmd.push(c);
}

pub fn cmd_backspace(){
    let mut cmd = COMMAND_LINE.lock();
    cmd.pop();
}

pub fn accpept_cmd(){
    let mut cmd = COMMAND_LINE.lock();
    let run: String = cmd.to_string();
    print!("\n");
    unsafe { exec_str_as_cmd(run) };
    cmd.clear();
    begin_new_cmd_line();
}

pub fn begin_new_cmd_line(){
    print!("\n$ ");
}

///This will execute any given string as a command.
///
///Unsafe because the user is responsible for string validation.
pub unsafe fn exec_str_as_cmd(str: String) -> i32{
    if str == "" {
        return 0;
    }

    let mut words = str.split_whitespace();

    let Some(program_name) = words.next() else {
        return 0;
    };

    let args: Vec<&str> = words.collect();

    for &(name, app_main) in termapps::APPS {
        if name == program_name {
            return app_main(&args);
        }
    }

    print!("No such command: {}", program_name);

    return 127;
}
