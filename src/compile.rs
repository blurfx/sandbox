use std::collections::HashMap;
use std::io::{self, Write};
use std::process::{Command};
use std::vec::Vec;
use nix::fcntl::{open, OFlag};
use nix::libc::{wait4, WNOHANG};
use nix::sys::stat::Mode;
use nix::unistd::{fork, dup2};
use nix::unistd::ForkResult::{Child, Parent};

pub struct CompileOption {
    pub language: String,
    pub input_path: String,
    pub output_path: String,
}

enum FlagToken {
    INPUT,
    OUTPUT
}

impl FlagToken {
    fn value(&self) -> &str {
        match *self {
            FlagToken::INPUT => "<INPUT>",
            FlagToken::OUTPUT => "<OUTPUT>",
        }
    }
}

fn get_compile_flags(language: &str) -> Option<(&str, Vec<&str>)> {
    let map: HashMap<&str, (&str, Vec<&str>)> = [
        ("cpp", ("g++", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
        ("c", ("gcc", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
    ].iter().cloned().collect();

    map.get(language).cloned()
}

pub fn compile(opt: CompileOption) -> i32 {
    let pid = unsafe { fork() };
    let mut code = 50000;

    match pid {
        Ok(Child) => {
            // redirect stdout, stderr to /dev/null
            let dev_null_read = open("/dev/null", OFlag::O_RDWR, Mode::empty()).unwrap();
            dup2(dev_null_read, 1).expect("dup2(STDOUT) failed");

            let dev_null_write = open("/dev/null", OFlag::O_WRONLY, Mode::empty()).unwrap();
            dup2(dev_null_write, 2).expect("dup2(STDERR) failed");

            // get compiler and compile flags
            let compiler: &str;
            let compile_args: Vec<&str>;
            match get_compile_flags(&opt.language){
                Some(flags) => {
                    compiler = flags.0;
                    compile_args = flags.1;
                }
                _ => {
                    panic!("unsupported language: {}", opt.language);
                }
            }

            // set varaiable as immutable
            let mut command = Command::new(compiler);
            for arg in compile_args {
                match arg {
                    _ if arg == FlagToken::INPUT.value() => command.arg(&opt.input_path),
                    _ if arg == FlagToken::OUTPUT.value() => command.arg(&opt.output_path),
                    _ => command.arg(arg),
                };
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
        Ok(Parent { child }) => {
            let mut status = 0;
            let mut rusage= std::mem::MaybeUninit::zeroed();
            loop {
                let child_pid = child.as_raw();
                let wait_result = unsafe { wait4(child_pid, & mut status, WNOHANG, rusage.as_mut_ptr()) };
                if wait_result == 0 {
                    continue;
                }

                code = status;
                break;
            }
        }
        _ => {
        }
    }

    code
}