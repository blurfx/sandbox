use std::convert::TryInto;
use std::ffi::CString;

use nix::libc::{waitpid, WNOHANG, getrusage, RUSAGE_CHILDREN, usleep, setrlimit, rlimit, self};
use nix::unistd::fork;
use nix::unistd::ForkResult::{Child, Parent};
use crate::exit_code::ExitCode;

pub struct ResourceLimit {
    pub memory: u64,
    pub time: u64,
}

#[repr(u32)]
enum Resource {
    AddressSpace = libc::RLIMIT_AS as u32,
    CPUTime = libc::RLIMIT_CPU as u32,
    CoreDump = libc::RLIMIT_CORE as u32,
}

fn set_resource_limit(resource: Resource, value: u64) -> Result<(), nix::Error> {
    let resource_id = (resource as u32).try_into().unwrap();
    let ret = unsafe {
        setrlimit(resource_id, &rlimit {
            rlim_cur: value,
            rlim_max: value,
        })
    };
    if ret == 0 {
        Ok(())
    } else {
        Err(nix::Error::last())
    }
}

pub fn execute(binary: &str, args: Vec<&str>, limits: Option<ResourceLimit>) -> i32 {
    let pid = unsafe { fork() };

    match pid {
        Ok(Child) => {
            let path = CString::new(binary).unwrap();

            let args = args.iter()
                .map(|arg| CString::new(arg.clone()).unwrap())
                .collect::<Vec<CString>>();

            let mut envs: Vec<CString> = vec![];
            for (name, value) in std::env::vars() {
                envs.push(CString::new(format!("{}={}", name, value)).unwrap());
            }

            println!("path: {:?}", path);

            if limits.is_some() {
                let limits = limits.unwrap();

                if set_resource_limit(Resource::AddressSpace, limits.memory).is_err() {
                    panic!("set memory limit failed");
                }

                if set_resource_limit(Resource::CPUTime, limits.time).is_err() {
                    panic!("set cpu time limit failed");
                }
            }

            if set_resource_limit(Resource::CoreDump, 0).is_err() {
                panic!("set core dump size limit failed")
            }

            match nix::unistd::execv(&path, args.as_ref()) {
                Ok(_) => {
                    println!("execve ok");
                    0
                }
                Err(e) => {
                    println!("execve failed: {}", e);
                    50000
                }
            }
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