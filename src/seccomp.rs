use std::ptr;

#[derive(Debug, Clone)]
pub enum SyscallFilterAction {
    Allow,
    Kill,
    Err(u32),
}

impl SyscallFilterAction {
    pub fn to_seccomp_action(&self) -> u32 {
        match self {
            SyscallFilterAction::Allow => seccomp_sys::SCMP_ACT_ALLOW,
            SyscallFilterAction::Kill => seccomp_sys::SCMP_ACT_KILL,
            SyscallFilterAction::Err(errno) => errno.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyscallFilter {
    pub default_action: SyscallFilterAction,
    pub rules: Vec<(String, SyscallFilterAction)>,
    pub context: *mut seccomp_sys::scmp_filter_ctx,
}

impl Default for SyscallFilter {
    fn default() -> Self {
        SyscallFilter {
            default_action: SyscallFilterAction::Kill,
            rules: vec![],
            context: ptr::null_mut(),
        }
    }
}

impl Drop for SyscallFilter {
    fn drop(&mut self) {
        unsafe {
            seccomp_sys::seccomp_release(self.context);
        }
    }
}

impl SyscallFilter {
    pub fn new() -> Self {
        let mut filter = SyscallFilter::default();

        filter.default_action(SyscallFilterAction::Allow);

        filter.context = unsafe {
            seccomp_sys::seccomp_init(filter.default_action.to_seccomp_action())
        };

        let denied_calls = vec![
            "_sysctl",
            "acct",
            "add_key",
            "bpf",
            "chroot",
            "clock_adjtime",
            "clock_settime",
            "clone",
            "connect",
            "create_module",
            "delete_module",
            "finit_module",
            "fork",
            "get_kernel_syms",
            "get_mempolicy",
            "getdents",
            "getdents64",
            "init_module",
            "ioperm",
            "iopl",
            "kcmp",
            "kexec_file_load",
            "kexec_load",
            "keyctl",
            "lookup_dcookie",
            "mbind",
            "mount",
            "move_pages",
            "name_to_handle_at",
            "nfsservctl",
            "open_by_handle_at",
            "perf_event_open",
            "personality",
            "pivot_root",
            "process_vm_readv",
            "process_vm_writev",
            "ptrace",
            "query_module",
            "quotactl",
            "reboot",
            "request_key",
            "set_mempolicy",
            "setns",
            "setrlimit",
            "settimeofday",
            "swapoff",
            "swapon",
            "sysfs",
            "umount2",
            "unshare",
            "uselib",
            "userfaultfd",
            "ustat",
            "vfork",
        ];

        for syscall_name in denied_calls {
            filter.add(syscall_name, SyscallFilterAction::Kill);
        }

        filter
    }

    pub fn default_action(&mut self, action: SyscallFilterAction) -> &mut Self {
        self.default_action = action;
        self
    }

    pub fn add(&mut self, syscall: &str, action: SyscallFilterAction) -> &mut Self {
        self.rules.push((syscall.to_string(), action));
        self
    }

    pub fn load(&self) {
        unsafe {
            seccomp_sys::seccomp_load(self.context);
        }
    }
}