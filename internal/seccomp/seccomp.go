package seccomp

import (
	"errors"
	"fmt"
	"syscall"
	"unsafe"

	"golang.org/x/sys/unix"
)

// BPF instruction constants.
const (
	bpfLd  = 0x00
	bpfW   = 0x00
	bpfAbs = 0x20
	bpfJmp = 0x05
	bpfJeq = 0x10
	bpfK   = 0x00
	bpfRet = 0x06

	// Seccomp return values.
	seccompRetKillProcess = 0x80000000
	seccompRetAllow       = 0x7fff0000

	// seccomp data offsets (for x86_64).
	offsetNR   = 0 // offset of syscall number in seccomp_data
	offsetArch = 4 // offset of arch in seccomp_data

	// Architecture value for x86_64.
	auditArchX8664 = 0xc000003e
)

// sockFprog is the structure for seccomp filter.
type sockFprog struct {
	Len    uint16
	Filter *sockFilter
}

// sockFilter is a single BPF instruction.
type sockFilter struct {
	Code uint16
	Jt   uint8
	Jf   uint8
	K    uint32
}

//nolint:funlen
func allowedSyscalls() []uint32 {
	return []uint32{
		// File I/O (basic)
		unix.SYS_READ,
		unix.SYS_WRITE,
		unix.SYS_OPEN,
		unix.SYS_OPENAT,
		unix.SYS_CLOSE,
		unix.SYS_LSEEK,
		unix.SYS_FSTAT,
		unix.SYS_STAT,
		unix.SYS_LSTAT,
		unix.SYS_NEWFSTATAT,
		unix.SYS_ACCESS,
		unix.SYS_FACCESSAT,
		unix.SYS_FACCESSAT2,
		unix.SYS_READV,
		unix.SYS_WRITEV,
		unix.SYS_PREAD64,
		unix.SYS_PWRITE64,
		unix.SYS_DUP,
		unix.SYS_DUP2,
		unix.SYS_DUP3,
		unix.SYS_FCNTL,
		unix.SYS_FADVISE64,
		unix.SYS_READLINK,
		unix.SYS_READLINKAT,
		unix.SYS_GETDENTS64,
		unix.SYS_GETCWD,

		// Memory management
		unix.SYS_BRK,
		unix.SYS_MMAP,
		unix.SYS_MUNMAP,
		unix.SYS_MPROTECT,
		unix.SYS_MREMAP,
		unix.SYS_MADVISE,

		// Process exit
		unix.SYS_EXIT,
		unix.SYS_EXIT_GROUP,

		// Exec (needed for initial program execution, safe because pivot_root is applied)
		unix.SYS_EXECVE,

		// Signals
		unix.SYS_RT_SIGACTION,
		unix.SYS_RT_SIGPROCMASK,
		unix.SYS_RT_SIGRETURN,
		unix.SYS_SIGALTSTACK,

		// Thread/process info (read-only)
		unix.SYS_GETPID,
		unix.SYS_GETTID,
		unix.SYS_GETUID,
		unix.SYS_GETGID,
		unix.SYS_GETEUID,
		unix.SYS_GETEGID,
		unix.SYS_GETPPID,
		unix.SYS_GETPGRP,
		unix.SYS_GETGROUPS,
		unix.SYS_GETRLIMIT,
		unix.SYS_PRLIMIT64,

		// Time
		unix.SYS_CLOCK_GETTIME,
		unix.SYS_CLOCK_GETRES,
		unix.SYS_GETTIMEOFDAY,
		unix.SYS_NANOSLEEP,
		unix.SYS_CLOCK_NANOSLEEP,
		unix.SYS_TIME,

		// Random
		unix.SYS_GETRANDOM,

		// Architecture/CPU
		unix.SYS_ARCH_PRCTL,
		unix.SYS_PRCTL,

		// Futex (for threading support in libc)
		unix.SYS_FUTEX,
		unix.SYS_SET_TID_ADDRESS,
		unix.SYS_SET_ROBUST_LIST,
		unix.SYS_GET_ROBUST_LIST,

		// Polling/select (for I/O)
		unix.SYS_POLL,
		unix.SYS_PPOLL,
		unix.SYS_SELECT,
		unix.SYS_PSELECT6,
		unix.SYS_EPOLL_CREATE,
		unix.SYS_EPOLL_CREATE1,
		unix.SYS_EPOLL_CTL,
		unix.SYS_EPOLL_WAIT,
		unix.SYS_EPOLL_PWAIT,

		// Misc necessary for C/C++ runtime
		unix.SYS_UNAME,
		unix.SYS_SYSINFO,
		unix.SYS_FLOCK,
		unix.SYS_UMASK,
		unix.SYS_IOCTL, // needed for terminal detection
		unix.SYS_PIPE,  // needed for some programs
		unix.SYS_PIPE2,
		unix.SYS_EVENTFD,
		unix.SYS_EVENTFD2,

		// Memory locking (usually fails but shouldn't crash)
		unix.SYS_MLOCK,
		unix.SYS_MUNLOCK,

		// Statx (modern stat)
		unix.SYS_STATX,

		// rseq (restartable sequences - needed by modern glibc)
		unix.SYS_RSEQ,

		// sched_* (scheduling - read operations)
		unix.SYS_SCHED_YIELD,
		unix.SYS_SCHED_GETAFFINITY,

		// Misc
		unix.SYS_MEMBARRIER,
		unix.SYS_GETRUSAGE,
	}
}

func buildFilter() []sockFilter {
	// Build the filter
	// Structure:
	// 1. Load architecture
	// 2. Check if x86_64, otherwise kill
	// 3. Load syscall number
	// 4. For each allowed syscall: if match, allow
	// 5. Default: kill

	var filter []sockFilter

	// Load architecture (offsetArch = 4)
	filter = append(filter, sockFilter{
		Code: bpfLd | bpfW | bpfAbs,
		K:    offsetArch,
	})

	// Check architecture - if not x86_64, kill
	// JEQ arch, skip 1 instruction (continue), else jump to kill
	filter = append(filter, sockFilter{
		Code: bpfJmp | bpfJeq | bpfK,
		Jt:   1, // if true, skip to next (load syscall nr)
		Jf:   0, // will be updated to jump to kill
		K:    auditArchX8664,
	})

	// Kill for wrong architecture
	filter = append(filter, sockFilter{
		Code: bpfRet | bpfK,
		K:    seccompRetKillProcess,
	})

	// Load syscall number (offsetNR = 0)
	filter = append(filter, sockFilter{
		Code: bpfLd | bpfW | bpfAbs,
		K:    offsetNR,
	})

	allowed := allowedSyscalls()
	if len(allowed) == 0 {
		return nil
	}

	// For each allowed syscall, add a check
	// Calculate jumps:
	// - If match: jump to ALLOW (which is after all checks and the default KILL)
	// - If no match: continue to next check (or KILL if last)
	var numAllowed uint8
	for range allowed {
		if numAllowed == ^uint8(0) {
			return nil
		}
		numAllowed++
	}
	var i uint8
	for _, nr := range allowed {
		jt := numAllowed - i // jump to ALLOW
		filter = append(filter, sockFilter{
			Code: bpfJmp | bpfJeq | bpfK,
			Jt:   jt,
			Jf:   0, // continue to next check
			K:    nr,
		})
		i++
	}

	// Default: KILL
	filter = append(filter, sockFilter{
		Code: bpfRet | bpfK,
		K:    seccompRetKillProcess,
	})

	// ALLOW
	filter = append(filter, sockFilter{
		Code: bpfRet | bpfK,
		K:    seccompRetAllow,
	})

	return filter
}

// Apply applies the seccomp filter to the current process.
func Apply() error {
	filter := buildFilter()
	if len(filter) == 0 {
		return errors.New("seccomp filter is empty")
	}

	var filterLen uint16
	for range filter {
		if filterLen == ^uint16(0) {
			return fmt.Errorf("seccomp filter too large: %d", len(filter))
		}
		filterLen++
	}

	prog := &sockFprog{
		Len:    filterLen,
		Filter: &filter[0],
	}

	// Set NO_NEW_PRIVS first (required for seccomp without CAP_SYS_ADMIN)
	if _, _, errno := syscall.Syscall6(
		unix.SYS_PRCTL,
		unix.PR_SET_NO_NEW_PRIVS,
		1, 0, 0, 0, 0,
	); errno != 0 {
		return errno
	}

	// Apply the seccomp filter
	if _, _, errno := syscall.Syscall(
		unix.SYS_SECCOMP,
		uintptr(unix.SECCOMP_SET_MODE_FILTER),
		0,
		uintptr(unsafe.Pointer(prog)),
	); errno != 0 {
		return errno
	}

	return nil
}

// Disable disables seccomp (only for testing purposes, must be called before Apply).
func Disable() error {
	return nil
}
