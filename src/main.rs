use night::executor::local::execution as exec;
use std::process::Command;

fn sys_info() {
    let result = Command::new("ls").output().expect("normal");
    let str_1 = String::from_utf8(result.stdout);
    println!("{str_1:?}");
}

fn main() {
    println!("Hello, world!");
    exec(14);
    sys_info();
}
