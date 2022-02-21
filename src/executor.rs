use std::time::Duration;

use nix::libc::{waitpid, WNOHANG, getrusage, RUSAGE_CHILDREN, usleep};
use nix::unistd::fork;
use nix::unistd::ForkResult::{Child, Parent};

use crate::exit_code::ExitCode;
use crate::process::{Process, Resource, Directory};

#[derive(Debug)]
struct ResourceUsage {
    pub user_time: Duration,
    pub cpu_time: Duration,
    pub memory: u64,
}

pub struct ResourceLimit {
    pub memory: u64,
    pub time: u64,
}

pub struct ExecuteOption {
    pub envs: Option<Vec<String>>,
    pub limits: Option<ResourceLimit>,
    pub input_path: Option<String>,
    pub directory: Option<Directory>,
    pub use_syscall: bool,
}

pub fn execute(binary: &str, args: Vec<&str>, option: ExecuteOption) -> i32 {
    let pid = unsafe { fork() };

    match pid {
        Ok(Child) => {
            let mut process = Process::new(binary.to_string()).args(args);

            if option.envs.is_some() {
                process = process.envs(option.envs.unwrap());
            }

            if option.limits.is_some() {
                let limits = option.limits.unwrap();

                process = process
                    .limit(Resource::AddressSpace, limits.memory)
                    .limit(Resource::CPUTime, limits.time)
                    .limit(Resource::CoreDump, 0);
            }

            if option.directory.is_some() {
                process = process.dir(option.directory.unwrap());
            }

            if option.input_path.is_some() {
                process = process.stdin(option.input_path.unwrap());
            }

            process = process.use_syscall_filter(option.use_syscall);

            process.run()
        }
        Ok(Parent{ child }) => {
            let mut status = 0;
            let mut usage = std::mem::MaybeUninit::uninit();

            loop {
                let child_pid = child.as_raw();
                let wait_result = unsafe { waitpid(child_pid, & mut status, WNOHANG) };
                let rusage = unsafe {
                    match getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
                        true => usage.assume_init(),
                        false => {
                            panic!("getrusage failed");
                        },
                    }
                };

                let resource_usage = ResourceUsage {
                    user_time: Duration::new(rusage.ru_utime.tv_sec as u64, rusage.ru_utime.tv_usec as u32),
                    cpu_time: Duration::new(rusage.ru_stime.tv_sec as u64, rusage.ru_stime.tv_usec as u32),
                    memory: rusage.ru_maxrss as u64,
                };

                if wait_result != 0 {
                    println!("{:?}", rusage);
                    println!("{:?}", resource_usage);
                    println!("exit code : {}", status);
                    break;
                }
                unsafe { usleep(10000); }
            }

            status
        }
        Err(err) => {
            eprintln!("{:?}", err);
            return ExitCode::UNKNOWN as i32;
        }
    }
}