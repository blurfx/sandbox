use std::time::Duration;

use nix::libc::{RUSAGE_CHILDREN, WNOHANG, getrusage, usleep, waitpid};
use nix::unistd::ForkResult::{Child, Parent};
use nix::unistd::fork;

use crate::exit_code::ExitCode;
use crate::process::{Directory, Process, Resource};

#[derive(Debug)]
pub struct ResourceUsage {
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
    pub output_path: Option<String>,
    pub directory: Option<Directory>,
    pub use_syscall: bool,
}

pub struct ExecuteResult {
    pub exit_code: i32,
    pub rusage: ResourceUsage,
}

pub fn execute(binary: &str, args: Vec<&str>, option: ExecuteOption) -> ExecuteResult {
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
                    .limit(Resource::AddressSpace, (limits.memory * 1024) * 2) // to bytes, 2x for safety
                    .limit(Resource::CPUTime, (limits.time + 1000) / 1000) // to seconds, +1 sec for safety
                    .limit(Resource::CoreDump, 0);
            }

            if option.directory.is_some() {
                process = process.dir(option.directory.unwrap());
            }

            if option.input_path.is_some() {
                process = process.stdin(option.input_path.unwrap());
            }

            if option.output_path.is_some() {
                process = process.stdout(option.output_path.unwrap());
            }

            process = process.use_syscall_filter(option.use_syscall);

            ExecuteResult {
                exit_code: process.run(),
                rusage: ResourceUsage {
                    user_time: Duration::ZERO,
                    cpu_time: Duration::ZERO,
                    memory: 0,
                },
            }
        }
        Ok(Parent { child }) => {
            let mut status = 0;
            loop {
                let wait_result = unsafe { waitpid(child.as_raw(), &mut status, WNOHANG) };

                if wait_result != 0 {
                    break;
                }
                unsafe {
                    usleep(10000);
                }
            }

            let mut usage = std::mem::MaybeUninit::uninit();
            let rusage = unsafe {
                match getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
                    true => usage.assume_init(),
                    false => {
                        panic!("getrusage failed");
                    }
                }
            };

            let resource_usage = ResourceUsage {
                user_time: Duration::new(
                    rusage.ru_utime.tv_sec as u64,
                    (rusage.ru_utime.tv_usec * 1000) as u32,
                ),
                cpu_time: Duration::new(
                    rusage.ru_stime.tv_sec as u64,
                    (rusage.ru_stime.tv_usec * 1000) as u32,
                ),
                memory: rusage.ru_maxrss as u64,
            };

            ExecuteResult {
                exit_code: status,
                rusage: resource_usage,
            }
        }
        Err(err) => {
            return ExecuteResult {
                exit_code: ExitCode::Unknown as i32,
                rusage: ResourceUsage {
                    user_time: Duration::ZERO,
                    cpu_time: Duration::ZERO,
                    memory: 0,
                },
            };
        }
    }
}
