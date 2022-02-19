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

impl SyscallFilter {
    pub fn new() -> Self {
        let mut filter = SyscallFilter::default();

        filter.default_action(SyscallFilterAction::Allow);

        filter.context = unsafe {
            seccomp_sys::seccomp_init(filter.default_action.to_seccomp_action())
        };

        filter.add("fork", SyscallFilterAction::Kill);
        filter.add("vfork", SyscallFilterAction::Kill);
        filter.add("clone", SyscallFilterAction::Kill);

        filter.add("clone", SyscallFilterAction::Kill);

        filter.add("chroot", SyscallFilterAction::Kill);
        filter.add("setrlimit", SyscallFilterAction::Kill);

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
}