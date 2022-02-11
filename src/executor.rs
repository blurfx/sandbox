use std::io::{self, Write};
use std::process::Command;

use nix::libc::{waitpid, WNOHANG, getrusage, RUSAGE_CHILDREN, usleep};
use nix::unistd::fork;
use nix::unistd::ForkResult::{Child, Parent};


pub fn execute(binary: &str, args: Vec<&str>) -> i32 {
    let pid = unsafe { fork() };
    let mut code = 50000;

    match pid {
        Ok(Child) => {
            let mut command = Command::new(binary);
            for arg in args {
                command.arg(arg);
            }

            match command.output() {
                Ok(o) => {
                    io::stdout().write_all(&o.stdout).unwrap();
                    io::stderr().write_all(&o.stderr).unwrap();
                    std::process::exit(o.status.code().unwrap());
                }
                Err(e) => {
                    eprintln!("{:?}", e);
                    std::process::exit(50000);
                }
            }
        }
        Ok(Parent{ child }) => {
            let mut status = 0;
            let mut usage = std::mem::MaybeUninit::uninit();
            loop {
                let child_pid = child.as_raw();
                let wait_result = unsafe { waitpid(child_pid, & mut status, WNOHANG) };
                let rusage_result = unsafe {
                    match getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
                        true => Some(usage.assume_init()),
                        false => None,
                    }
                };
                if rusage_result.is_some() {
                    println!("{:?}", rusage_result.unwrap());
                }
                if wait_result == 0 {
                    unsafe { usleep(10000); }
                    continue;
                }
                println!("exit code : {}", status);
                code = status;
                break;
            }
        }
        Err(err) => {
            eprintln!("{:?}", err);
            code = 50000;
        }
    }

    code
}