package sandbox

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"

	"sandbox/internal/rlimit"
	"sandbox/internal/seccomp"

	"golang.org/x/sys/unix"
)

const (
	defaultUmask                 = 0o022
	virtualMemoryLimitMultiplier = 2
)

// ChildConfig holds configuration for the child process.
type ChildConfig struct {
	RootDir       string `json:"root_dir"`
	WorkDir       string `json:"work_dir"`
	Binary        string `json:"binary"`
	InputFile     string `json:"input_file"`
	OutputFile    string `json:"output_file"`
	SyncReadyFD   int    `json:"sync_ready_fd"`
	SyncGoFD      int    `json:"sync_go_fd"`
	TimeLimit     int64  `json:"time_limit"`   // seconds
	MemoryLimit   int64  `json:"memory_limit"` // bytes
	DiskLimit     int64  `json:"disk_limit"`   // bytes
	StackLimit    int64  `json:"stack_limit"`  // bytes
	OpenFiles     int64  `json:"open_files"`
	MaxProcesses  int64  `json:"max_processes"`
	CgroupPath    string `json:"cgroup_path"`
	HasInputFile  bool   `json:"has_input_file"`
	HasOutputFile bool   `json:"has_output_file"`
	NoSeccomp     bool   `json:"no_seccomp"`
	DropUID       bool   `json:"drop_uid"`
	RunUID        int    `json:"run_uid"`
	RunGID        int    `json:"run_gid"`
	DisableTHP    bool   `json:"disable_thp"`
	DisableASLR   bool   `json:"disable_aslr"`
}

// RunChild runs the child process logic.
func RunChild(argsJSON string) error {
	// Parse config
	var cfg ChildConfig
	if err := json.Unmarshal([]byte(argsJSON), &cfg); err != nil {
		return fmt.Errorf("failed to parse child config: %w", err)
	}

	// Setup root filesystem
	if err := setupRootFS(&cfg); err != nil {
		return fmt.Errorf("failed to setup root filesystem: %w", err)
	}

	// Apply resource limits
	if err := applyLimits(&cfg); err != nil {
		return fmt.Errorf("failed to apply limits: %w", err)
	}

	// Setup I/O redirection
	if err := setupIO(&cfg); err != nil {
		return fmt.Errorf("failed to setup I/O: %w", err)
	}

	// Drop privileges (only relevant when the sandbox process runs as real root)
	if cfg.DropUID {
		if err := dropPrivileges(&cfg); err != nil {
			return fmt.Errorf("failed to drop privileges: %w", err)
		}
	}

	// Optional determinism knobs: do this after filesystem setup/limits and before
	// installing seccomp and execing the user program.
	applyDeterminism(&cfg)

	if err := syncWithParent(&cfg); err != nil {
		return fmt.Errorf("failed to sync with parent: %w", err)
	}

	// Apply seccomp filter (must be last before exec)
	if !cfg.NoSeccomp {
		if err := seccomp.Apply(); err != nil {
			return fmt.Errorf("failed to apply seccomp: %w", err)
		}
	}

	// Execute the user program
	return execProgram(&cfg)
}

func syncWithParent(cfg *ChildConfig) error {
	if cfg.SyncReadyFD <= 0 || cfg.SyncGoFD <= 0 {
		return nil
	}

	ready := os.NewFile(uintptr(cfg.SyncReadyFD), "sync-ready")
	if ready == nil {
		return fmt.Errorf("failed to open sync-ready fd %d", cfg.SyncReadyFD)
	}
	if _, err := ready.Write([]byte{1}); err != nil {
		_ = ready.Close()
		return err
	}
	if err := ready.Close(); err != nil {
		return err
	}

	goSignal := os.NewFile(uintptr(cfg.SyncGoFD), "sync-go")
	if goSignal == nil {
		return fmt.Errorf("failed to open sync-go fd %d", cfg.SyncGoFD)
	}
	defer goSignal.Close()

	buf := []byte{0}
	if _, err := io.ReadFull(goSignal, buf); err != nil {
		return err
	}
	return nil
}

func applyDeterminism(cfg *ChildConfig) {
	if cfg.DisableTHP {
		// Best-effort: on older kernels this can return EINVAL.
		_ = unix.Prctl(unix.PR_SET_THP_DISABLE, 1, 0, 0, 0)
	}

	if cfg.DisableASLR {
		const addrNoRandomize = 0x0040000

		// personality(0xffffffff) returns the current personality without changing it.
		current, _, errno := syscall.Syscall(syscall.SYS_PERSONALITY, ^uintptr(0), 0, 0)
		if errno == 0 {
			_, _, _ = syscall.Syscall(syscall.SYS_PERSONALITY, current|addrNoRandomize, 0, 0)
		}
	}
}

func dropPrivileges(cfg *ChildConfig) error {
	if cfg.RunUID <= 0 || cfg.RunGID <= 0 {
		return fmt.Errorf("invalid run uid/gid: %d/%d", cfg.RunUID, cfg.RunGID)
	}

	// Restrict supplementary groups first.
	if err := unix.Setgroups([]int{cfg.RunGID}); err != nil {
		// Fall back to clearing groups if setting is not permitted.
		_ = unix.Setgroups([]int{})
	}

	if err := unix.Setresgid(cfg.RunGID, cfg.RunGID, cfg.RunGID); err != nil {
		return err
	}
	return unix.Setresuid(cfg.RunUID, cfg.RunUID, cfg.RunUID)
}

// setupRootFS sets up the root filesystem using pivot_root.
func setupRootFS(cfg *ChildConfig) error {
	// Make everything in our mount namespace private
	if err := unix.Mount("", "/", "", unix.MS_PRIVATE|unix.MS_REC, ""); err != nil {
		return fmt.Errorf("failed to make / private: %w", err)
	}

	// Bind mount the new root to itself (required for pivot_root)
	// Disk limit is enforced via RLIMIT_FSIZE instead of tmpfs
	if err := unix.Mount(cfg.RootDir, cfg.RootDir, "", unix.MS_BIND|unix.MS_REC, ""); err != nil {
		return fmt.Errorf("failed to bind mount new root: %w", err)
	}

	// Create the old_root directory inside the new root
	oldRoot := filepath.Join(cfg.RootDir, "old_root")
	if err := os.MkdirAll(oldRoot, 0750); err != nil {
		return fmt.Errorf("failed to create old_root: %w", err)
	}

	// Pivot root
	if err := unix.PivotRoot(cfg.RootDir, oldRoot); err != nil {
		return fmt.Errorf("failed to pivot_root: %w", err)
	}

	// Change to new root
	if err := os.Chdir("/"); err != nil {
		return fmt.Errorf("failed to chdir to /: %w", err)
	}

	// Mount proc filesystem
	if err := unix.Mount("proc", "/proc", "proc", unix.MS_NOSUID|unix.MS_NOEXEC|unix.MS_NODEV, ""); err != nil {
		return fmt.Errorf("failed to mount /proc: %w", err)
	}

	// Create and mount minimal /dev
	if err := setupDev(); err != nil {
		return fmt.Errorf("failed to setup /dev: %w", err)
	}

	// Unmount old root
	if err := unix.Unmount("/old_root", unix.MNT_DETACH); err != nil {
		return fmt.Errorf("failed to unmount old_root: %w", err)
	}

	// Remove old root directory
	_ = os.RemoveAll("/old_root")

	// Change to work directory
	if err := os.Chdir(cfg.WorkDir); err != nil {
		return fmt.Errorf("failed to chdir to work dir: %w", err)
	}

	// Set umask
	unix.Umask(defaultUmask)

	// Set hostname
	_ = unix.Sethostname([]byte("sandbox"))

	return nil
}

// setupDev creates minimal device nodes.
func setupDev() error {
	// Mount a fresh tmpfs for /dev
	if err := unix.Mount("tmpfs", "/dev", "tmpfs", unix.MS_NOSUID|unix.MS_NOEXEC, "size=64k,mode=755"); err != nil {
		return err
	}

	// Create necessary device nodes
	devices := []struct {
		path  string
		mode  uint32
		major uint32
		minor uint32
	}{
		{"/dev/null", syscall.S_IFCHR | 0666, 1, 3},
		{"/dev/zero", syscall.S_IFCHR | 0666, 1, 5},
		{"/dev/urandom", syscall.S_IFCHR | 0444, 1, 9},
		{"/dev/random", syscall.S_IFCHR | 0444, 1, 8},
	}

	for _, dev := range devices {
		devNum := unix.Mkdev(dev.major, dev.minor)
		maxInt := uint64(int(^uint(0) >> 1))
		if devNum > maxInt {
			return fmt.Errorf("device number overflows int: %d", devNum)
		}
		if err := unix.Mknod(dev.path, dev.mode, int(devNum)); err != nil {
			// Try to create a regular file as fallback
			f, createErr := os.Create(dev.path)
			if createErr == nil {
				_ = f.Close()
			}
		}
	}

	// Create /dev/fd symlink
	links := []struct {
		target string
		link   string
	}{
		{target: "/proc/self/fd", link: "/dev/fd"},
		{target: "/proc/self/fd/0", link: "/dev/stdin"},
		{target: "/proc/self/fd/1", link: "/dev/stdout"},
		{target: "/proc/self/fd/2", link: "/dev/stderr"},
	}
	for _, link := range links {
		if err := os.Symlink(link.target, link.link); err != nil && !os.IsExist(err) {
			return err
		}
	}

	return nil
}

// applyLimits applies resource limits using setrlimit.
func applyLimits(cfg *ChildConfig) error {
	// RLIMIT_NPROC is a per-UID limit. When we drop privileges (sudo mode) it can
	// easily be exceeded by the *existing* processes/threads of that user, which
	// then makes execve() fail with EAGAIN ("resource temporarily unavailable").
	//
	// Prefer enforcing process/task limits via the pids cgroup controller; it's
	// per-sandbox rather than global.
	processLimit := cfg.MaxProcesses
	if cfg.CgroupPath != "" || cfg.DropUID {
		processLimit = 0
	}

	limits := &rlimit.Limits{
		CPUTime:   cfg.TimeLimit + 1,                              // Add 1 second buffer
		Memory:    cfg.MemoryLimit * virtualMemoryLimitMultiplier, // Virtual memory limit (generous for safety)
		FileSize:  cfg.DiskLimit,
		Stack:     cfg.StackLimit,
		OpenFiles: cfg.OpenFiles,
		Processes: processLimit,
		Core:      0, // Disable core dumps
	}

	return rlimit.Apply(limits)
}

// setupIO sets up stdin/stdout redirection.
func setupIO(cfg *ChildConfig) error {
	// Setup stdin
	if cfg.HasInputFile {
		inputFile, err := os.Open(cfg.InputFile)
		if err != nil {
			return fmt.Errorf("failed to open input file: %w", err)
		}

		// Duplicate input file to stdin
		if dupErr := unix.Dup2(int(inputFile.Fd()), int(os.Stdin.Fd())); dupErr != nil {
			_ = inputFile.Close()
			return fmt.Errorf("failed to dup2 stdin: %w", dupErr)
		}
		_ = inputFile.Close()
	}

	// Setup stdout
	if cfg.HasOutputFile {
		outputFile, err := os.Create(cfg.OutputFile)
		if err != nil {
			return fmt.Errorf("failed to create output file: %w", err)
		}

		// Duplicate output file to stdout
		if dupErr := unix.Dup2(int(outputFile.Fd()), int(os.Stdout.Fd())); dupErr != nil {
			_ = outputFile.Close()
			return fmt.Errorf("failed to dup2 stdout: %w", dupErr)
		}
		_ = outputFile.Close()
	}

	return nil
}

// execProgram executes the user program.
func execProgram(cfg *ChildConfig) error {
	// Prepare arguments
	args := []string{cfg.Binary}

	// Prepare environment (minimal)
	env := []string{
		"PATH=/bin:/usr/bin",
		"HOME=/tmp",
		"LANG=C.UTF-8",
	}

	// Execute the program
	//nolint:gosec // Executing a user-supplied program is the sandbox's core purpose.
	return syscall.Exec(cfg.Binary, args, env)
}
