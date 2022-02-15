use nix::libc::{waitpid, WNOHANG, getrusage, RUSAGE_CHILDREN, usleep};
use nix::unistd::fork;
use nix::unistd::ForkResult::{Child, Parent};

use crate::exit_code::ExitCode;
use crate::process::{Process, Resource};

pub struct ResourceLimit {
    pub memory: u64,
    pub time: u64,
}

pub fn execute(binary: &str, args: Vec<&str>, envs: Option<Vec<&str>>, limits: Option<ResourceLimit>) -> i32 {
    let pid = unsafe { fork() };

    match pid {
        Ok(Child) => {
            let mut process = Process::new(binary.to_string())
            .args(args);

            if envs.is_some() {
                process = process.envs(envs.unwrap());
            }

            if limits.is_some() {
                let limits = limits.unwrap();

                process = process
                    .limit(Resource::AddressSpace, limits.memory)
                    .limit(Resource::CPUTime, limits.time)
                    .limit(Resource::CoreDump, 0);
            }

            process.run()
        }
        Ok(Parent{ child }) => {
            let mut status = 0;
            let mut usage = std::mem::MaybeUninit::uninit();
            loop {
                let child_pid = child.as_raw();
                let wait_result = unsafe { waitpid(child_pid, & mut status, WNOHANG) };
                if wait_result == 0 {
                    unsafe { usleep(10000); }
                    continue;
                }

                let rusage_result = unsafe {
                    match getrusage(RUSAGE_CHILDREN, usage.as_mut_ptr()) == 0 {
                        true => Some(usage.assume_init()),
                        false => None,
                    }
                };

                println!("{:?}", rusage_result.unwrap());
                println!("exit code : {}", status);
                return status;
            }
        }
        Err(err) => {
            eprintln!("{:?}", err);
            return ExitCode::UNKNOWN as i32;
        }
    }
}