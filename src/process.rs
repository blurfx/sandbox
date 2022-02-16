use std::{convert::TryInto, ffi::CString};

use nix::libc;

#[repr(u32)]
pub enum Resource {
    AddressSpace = libc::RLIMIT_AS as u32,
    CPUTime = libc::RLIMIT_CPU as u32,
    CoreDump = libc::RLIMIT_CORE as u32,
}


#[derive(Debug, Clone)]
pub struct Process {
    path: CString,
    args: Vec<CString>,
    envs: Vec<CString>,
    limits: Vec<(i32, u64)>,
}

impl Process {
    pub fn new(path: String) -> Self {
        let path = CString::new(path).unwrap();

        Process {
            path,
            args: vec![],
            envs: vec![],
            limits: vec![],
        }
    }

    pub fn args(mut self, args: Vec<&str>) -> Self {
         self.args = args.iter()
                .map(|arg| CString::new(arg.clone()).unwrap())
                .collect::<Vec<CString>>();
        self
    }

    pub fn envs(mut self, envs: Vec<&str>) -> Self {
        self.envs = envs.iter()
                .map(|env| CString::new(format!("{}", env)).unwrap())
                .collect::<Vec<CString>>();
        self
    }

    pub fn limit(mut self, resource: Resource, value: u64) -> Self {
        self.limits.push((resource as i32, value));
        self
    }

    fn setrlimit(&self) {
        for (resource, value) in &self.limits {
            let ret = unsafe {
                libc::setrlimit((*resource).try_into().unwrap(), &libc::rlimit {
                    rlim_cur: *value,
                    rlim_max: *value,
                })
            };
            if ret != 0 {
                panic!("set resource limit failed");
            }
        }
    }

    pub fn run(&self) -> i32 {
        self.setrlimit();

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