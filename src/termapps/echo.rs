use crate::print;

pub fn main(args: &[&str]) -> i32 {
    for (index, arg) in args.iter().enumerate(){
        if index > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    return 0;
}
