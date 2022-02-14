use std::{convert::TryInto, ffi::CString};

use nix::libc;

#[repr(u32)]
pub enum Resource {
    AddressSpace = libc::RLIMIT_AS as u32,
    CPUTime = libc::RLIMIT_CPU as u32,
    CoreDump = libc::RLIMIT_CORE as u32,
}

pub struct Process {
    path: CString,
    args: Vec<CString>,
}

impl Process {
    pub fn new(path: String) -> Self {
        let path = CString::new(path).unwrap();

        Process {
            path,
            args: vec![],
        }
    }

    pub fn args(mut self, args: Vec<&str>) -> Self {
         self.args = args.iter()
                .map(|arg| CString::new(arg.clone()).unwrap())
                .collect::<Vec<CString>>();
        self
    }

    pub fn limit(self, resource: Resource, value: u64) -> Self {
        let resource_id = (resource as u32).try_into().unwrap();
        let ret = unsafe {
            libc::setrlimit(resource_id, &libc::rlimit {
                rlim_cur: value,
                rlim_max: value,
            })
        };
        if ret != 0 {
            panic!("set resource limit failed");
        }
        self
    }

    pub fn run(self) -> i32 {
        match nix::unistd::execv(&self.path, self.args.as_ref()) {
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
}