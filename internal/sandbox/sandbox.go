package sandbox

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"sandbox/internal/cgroup"
	"sandbox/internal/config"
	"sandbox/internal/result"
)

const (
	extraFilesBaseFD = 3
	syncReadyFD      = extraFilesBaseFD
	syncGoFD         = extraFilesBaseFD + 1

	minPidsLimitDuringSetup = 64
	childSyncTimeoutCap     = 5 * time.Second
)

// Sandbox manages the sandboxed execution of user programs.
type Sandbox struct {
	cfg     *config.Config
	cg      *cgroup.Cgroup
	rootDir string
	result  *result.Result
	runUID  int
	runGID  int
	dropUID bool
}

// New creates a new Sandbox instance.
func New(cfg *config.Config) *Sandbox {
	return &Sandbox{
		cfg:    cfg,
		result: result.New(),
	}
}

// Run executes the user program in the sandbox.
func (s *Sandbox) Run() *result.Result {
	// If we're running as real root, we'll run the sandbox process as root to set
	// up namespaces/filesystem, but drop privileges before executing the user program.
	s.configureRunUser()

	// Setup cgroup if enabled
	if !s.cfg.NoCgroup {
		cgroupName := fmt.Sprintf("sandbox-%d", os.Getpid())
		s.cg = cgroup.New(cgroupName)

		if err := s.setupCgroup(); err != nil {
			s.result.SetInternalError(fmt.Sprintf("cgroup setup failed: %v", err))
			return s.result
		}
		defer func() {
			if err := s.cg.Destroy(); err != nil {
				_, _ = fmt.Fprintf(os.Stderr, "failed to destroy cgroup: %v\n", err)
			}
		}()
	}

	// Prepare root filesystem
	if err := s.prepareRoot(); err != nil {
		s.result.SetInternalError(fmt.Sprintf("root preparation failed: %v", err))
		return s.result
	}
	defer s.cleanup()

	// Execute the child process
	if err := s.execute(); err != nil {
		// Error is already set in s.result by execute()
		return s.result
	}

	return s.result
}

func (s *Sandbox) configureRunUser() {
	if os.Geteuid() != 0 {
		s.dropUID = false
		return
	}

	// Prefer dropping to the invoking user when run via sudo, otherwise fall back
	// to nobody.
	s.runUID = 65534
	s.runGID = 65534
	s.dropUID = true

	if sudoUID := os.Getenv("SUDO_UID"); sudoUID != "" {
		if v, err := strconv.Atoi(sudoUID); err == nil && v > 0 {
			s.runUID = v
		}
	}
	if sudoGID := os.Getenv("SUDO_GID"); sudoGID != "" {
		if v, err := strconv.Atoi(sudoGID); err == nil && v > 0 {
			s.runGID = v
		}
	}
}

// setupCgroup creates and configures the cgroup.
func (s *Sandbox) setupCgroup() error {
	if err := s.cg.Create(); err != nil {
		return fmt.Errorf("failed to create cgroup: %w", err)
	}

	// Set memory limit
	if err := s.cg.SetMemoryLimit(s.cfg.MemoryLimitBytes()); err != nil {
		return fmt.Errorf("failed to set memory limit: %w", err)
	}

	// Disable swap
	_ = s.cg.SetMemorySwapLimit(0)

	// Set process limit
	setupPidsLimit := max(s.cfg.MaxProcesses, minPidsLimitDuringSetup)
	if err := s.cg.SetPidsLimit(setupPidsLimit); err != nil {
		return fmt.Errorf("failed to set pids limit: %w", err)
	}

	return nil
}

// prepareRoot prepares the root filesystem for the sandbox
// Note: tmpfs will be mounted in the child process within its mount namespace.
//
//nolint:gocognit
func (s *Sandbox) prepareRoot() error {
	// Create temporary directory for root
	rootDir, mkErr := os.MkdirTemp("", "sandbox-root-")
	if mkErr != nil {
		return fmt.Errorf("failed to create temp dir: %w", mkErr)
	}
	s.rootDir = rootDir
	if s.dropUID {
		// MkdirTemp uses 0700 by default; when the child drops privileges we must
		// allow it to traverse the new root.
		//nolint:gosec // The root directory must be traversable by the dropped UID inside the sandbox.
		if chmodErr := os.Chmod(s.rootDir, 0755); chmodErr != nil {
			return fmt.Errorf("failed to chmod root dir: %w", chmodErr)
		}
	}

	// Create directory structure
	dirs := []string{
		"bin", "lib", "lib64", "usr/lib", "usr/lib64",
		"proc", "dev", "tmp", "work", "etc",
	}
	for _, dir := range dirs {
		path := filepath.Join(s.rootDir, dir)
		//nolint:gosec // Rootfs directories must be readable/executable for the sandboxed process.
		if mkdirErr := os.MkdirAll(path, 0755); mkdirErr != nil {
			return fmt.Errorf("failed to create directory %s: %w", dir, mkdirErr)
		}
	}

	// Ensure /tmp is writable for the executed program.
	//nolint:gosec // Sandbox /tmp needs sticky-bit semantics for typical programs.
	if chmodErr := os.Chmod(filepath.Join(s.rootDir, "tmp"), 01777); chmodErr != nil {
		return fmt.Errorf("failed to chmod tmp: %w", chmodErr)
	}

	// When running as real root, the user program will drop privileges, so make
	// /work writable for that user (while keeping copied binaries root-owned).
	if s.dropUID {
		workDir := filepath.Join(s.rootDir, "work")
		if chownErr := os.Chown(workDir, s.runUID, s.runGID); chownErr != nil {
			return fmt.Errorf("failed to chown work dir: %w", chownErr)
		}
	}

	// Copy user binary
	srcBinary := s.cfg.Binary
	dstBinary := filepath.Join(s.rootDir, "work", "program")
	if copyErr := copyFile(srcBinary, dstBinary); copyErr != nil {
		return fmt.Errorf("failed to copy binary: %w", copyErr)
	}
	//nolint:gosec // The copied binary must be executable inside the sandbox.
	if chmodErr := os.Chmod(dstBinary, 0755); chmodErr != nil {
		return fmt.Errorf("failed to chmod binary: %w", chmodErr)
	}

	// Copy input file if specified
	if s.cfg.InputFile != "" {
		dstInput := filepath.Join(s.rootDir, "work", "input.txt")
		if copyErr := copyFile(s.cfg.InputFile, dstInput); copyErr != nil {
			return fmt.Errorf("failed to copy input file: %w", copyErr)
		}
	}

	// Copy required libraries
	if copyLibErr := s.copyLibraries(); copyLibErr != nil {
		return fmt.Errorf("failed to copy libraries: %w", copyLibErr)
	}

	// Create minimal /etc/passwd and /etc/group
	passwdPath := filepath.Join(s.rootDir, "etc", "passwd")
	if writeErr := os.WriteFile(
		passwdPath,
		[]byte("nobody:x:65534:65534:nobody:/:/bin/false\n"),
		0400,
	); writeErr != nil {
		return fmt.Errorf("failed to create /etc/passwd: %w", writeErr)
	}
	groupPath := filepath.Join(s.rootDir, "etc", "group")
	if writeErr := os.WriteFile(groupPath, []byte("nobody:x:65534:\n"), 0400); writeErr != nil {
		return fmt.Errorf("failed to create /etc/group: %w", writeErr)
	}
	if s.dropUID {
		if chownErr := os.Chown(passwdPath, s.runUID, s.runGID); chownErr != nil {
			return fmt.Errorf("failed to chown /etc/passwd: %w", chownErr)
		}
		if chownErr := os.Chown(groupPath, s.runUID, s.runGID); chownErr != nil {
			return fmt.Errorf("failed to chown /etc/group: %w", chownErr)
		}
	}

	return nil
}

// copyLibraries copies required shared libraries for the binary.
//
//nolint:gocognit
func (s *Sandbox) copyLibraries() error {
	// Get list of required libraries using ldd.
	//nolint:gosec // Running ldd on the user binary is used to discover dynamic dependencies for the sandbox rootfs.
	cmd := exec.CommandContext(context.Background(), "ldd", s.cfg.Binary)
	output, cmdErr := cmd.CombinedOutput()
	if cmdErr != nil {
		outStr := string(output)
		if strings.Contains(outStr, "not a dynamic executable") || strings.Contains(outStr, "statically linked") {
			return nil
		}
		return fmt.Errorf("ldd failed: %w: %s", cmdErr, strings.TrimSpace(outStr))
	}

	// Parse ldd output and copy libraries.
	libs := parseLddOutput(string(output))
	for _, lib := range libs {
		if lib == "" || lib == "linux-vdso.so.1" {
			continue
		}

		dstPath := filepath.Join(s.rootDir, "lib64", filepath.Base(lib))
		switch {
		case strings.HasPrefix(lib, "/lib64/"),
			strings.HasPrefix(lib, "/lib/"),
			strings.HasPrefix(lib, "/usr/lib64/"),
			strings.HasPrefix(lib, "/usr/lib/"):
			dstPath = filepath.Join(s.rootDir, strings.TrimPrefix(lib, "/"))
		}

		// Create parent directory.
		//nolint:gosec // Rootfs directories must be readable/executable for the sandboxed process.
		if mkdirErr := os.MkdirAll(filepath.Dir(dstPath), 0755); mkdirErr != nil {
			return fmt.Errorf("failed to create library directory %s: %w", filepath.Dir(dstPath), mkdirErr)
		}

		// Copy the library.
		if copyErr := copyFile(lib, dstPath); copyErr != nil {
			return fmt.Errorf("failed to copy library %s: %w", lib, copyErr)
		}
	}

	// Also copy the dynamic linker.
	linkers := []string{
		"/lib64/ld-linux-x86-64.so.2",
		"/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
	}
	for _, linker := range linkers {
		if _, statErr := os.Stat(linker); statErr != nil {
			continue
		}
		dstPath := filepath.Join(s.rootDir, strings.TrimPrefix(linker, "/"))
		//nolint:gosec // Rootfs directories must be readable/executable for the sandboxed process.
		if mkdirErr := os.MkdirAll(filepath.Dir(dstPath), 0755); mkdirErr != nil {
			return fmt.Errorf("failed to create linker directory %s: %w", filepath.Dir(dstPath), mkdirErr)
		}
		if copyErr := copyFile(linker, dstPath); copyErr != nil {
			return fmt.Errorf("failed to copy dynamic linker %s: %w", linker, copyErr)
		}
	}

	return nil
}

// execute runs the child process.
//
//nolint:gocognit,funlen
func (s *Sandbox) execute() error {
	readyR, readyW, pipeErr := os.Pipe()
	if pipeErr != nil {
		s.result.SetInternalError(fmt.Sprintf("failed to create sync pipe: %v", pipeErr))
		return pipeErr
	}
	defer func() { _ = readyR.Close() }()
	defer func() { _ = readyW.Close() }()

	goR, goW, pipeErr := os.Pipe()
	if pipeErr != nil {
		s.result.SetInternalError(fmt.Sprintf("failed to create sync pipe: %v", pipeErr))
		return pipeErr
	}
	defer func() { _ = goR.Close() }()
	defer func() { _ = goW.Close() }()

	childArgs, marshalErr := json.Marshal(s.getChildConfig(syncReadyFD, syncGoFD))
	if marshalErr != nil {
		s.result.SetInternalError(fmt.Sprintf("failed to serialize config: %v", marshalErr))
		return marshalErr
	}

	cmd := s.newChildCmd(childArgs)
	cmd.ExtraFiles = []*os.File{readyW, goR}
	cgroupFDFile, startedInCgroup := s.tryUseCgroupFD(cmd)
	wallTimer := startWallTimer(cmd, time.Duration(s.cfg.WallTimeLimit)*time.Millisecond)
	startTime := time.Now()

	if startErr := cmd.Start(); startErr != nil {
		if cgroupFDFile != nil {
			_ = cgroupFDFile.Close()
		}
		wallTimer.Stop()
		s.result.SetInternalError(fmt.Sprintf("failed to start child: %v", startErr))
		return startErr
	}
	if cgroupFDFile != nil {
		_ = cgroupFDFile.Close()
	}
	_ = readyW.Close()
	_ = goR.Close()

	if addErr := s.maybeAddToCgroup(cmd, startedInCgroup); addErr != nil {
		wallTimer.Stop()
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
		s.result.SetInternalError(fmt.Sprintf("failed to add process to cgroup: %v", addErr))
		return addErr
	}

	if syncErr := s.syncChildBeforeExec(readyR, goW); syncErr != nil {
		wallTimer.Stop()
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
		s.result.SetInternalError(fmt.Sprintf("failed to sync with child: %v", syncErr))
		return syncErr
	}

	poller := newMemoryPoller(s.cg)

	waitErr := cmd.Wait()
	wallTimer.Stop()
	wallTime := time.Since(startTime)

	var memoryPeakCgroup int64
	if poller != nil {
		memoryPeakCgroup = poller.Stop()
	}

	rusage := rusageFromProcessState(cmd.ProcessState)
	memoryPeakRUsage := memoryPeakFromRusage(rusage)
	cpuUsage := cpuUsageFromRusage(rusage)

	var oomKilled bool
	if s.cg != nil {
		if usage, cpuErr := s.cg.GetCPUUsage(); cpuErr == nil {
			cpuUsage = usage
		}
		if peak, peakErr := s.cg.GetMemoryPeak(); peakErr == nil && peak > 0 {
			memoryPeakCgroup = peak
		}
		if killed, oomErr := s.cg.IsOOMKilled(); oomErr == nil {
			oomKilled = killed
		}
	} else {
		memoryPeakCgroup = memoryPeakRUsage
	}

	memoryReport := memoryPeakCgroup
	if s.cfg.MemoryMetric == "rss" && memoryPeakRUsage > 0 {
		memoryReport = memoryPeakRUsage
	}
	s.result.SetResourceUsage(cpuUsage, wallTime.Milliseconds(), memoryReport)

	if s.applyLimitsAndTimeouts(oomKilled, wallTime, cpuUsage) {
		return nil
	}

	if waitErr != nil {
		s.applyWaitError(waitErr, memoryPeakCgroup)
		return nil
	}

	s.copyOutputBack()
	s.result.SetOK()
	return nil
}

func (s *Sandbox) syncChildBeforeExec(readyR *os.File, goW *os.File) error {
	if readyR == nil || goW == nil {
		return nil
	}

	readyTimeout := min(time.Duration(s.cfg.WallTimeLimit)*time.Millisecond, childSyncTimeoutCap)

	readyCh := make(chan error, 1)
	go func() {
		buf := []byte{0}
		_, err := readyR.Read(buf)
		readyCh <- err
	}()

	select {
	case err := <-readyCh:
		if err != nil {
			return err
		}
	case <-time.After(readyTimeout):
		return errors.New("timeout waiting for child readiness")
	}

	if s.cg != nil {
		if err := s.cg.SetPidsLimit(s.cfg.MaxProcesses); err != nil {
			return err
		}
	}

	if _, err := goW.Write([]byte{1}); err != nil {
		return err
	}
	return nil
}

const (
	bytesPerKiB             = int64(1024)
	microsecondsPerSecond   = int64(time.Second / time.Microsecond)
	microsecondsPerMilliSec = int64(time.Millisecond / time.Microsecond)
)

func (s *Sandbox) newChildCmd(childArgs []byte) *exec.Cmd {
	// Re-exec via /proc to avoid depending on path traversal permissions while UID
	// mappings are being established.
	//nolint:gosec // Execing the current binary (/proc/self/exe) is controlled and intentional.
	cmd := exec.CommandContext(context.Background(), "/proc/self/exe", "--child", "--child-args", string(childArgs))
	cmd.SysProcAttr = s.childSysProcAttr()
	cmd.Stderr = os.Stderr
	return cmd
}

func (s *Sandbox) childSysProcAttr() *syscall.SysProcAttr {
	if os.Geteuid() == 0 {
		// Running as real root: avoid CLONE_NEWUSER. It is unnecessary here and
		// combining it with CLONE_NEWNS breaks mount setup on Ubuntu with
		// kernel.apparmor_restrict_unprivileged_userns=1.
		return &syscall.SysProcAttr{
			Cloneflags: syscall.CLONE_NEWNS |
				syscall.CLONE_NEWPID |
				syscall.CLONE_NEWNET |
				syscall.CLONE_NEWUTS |
				syscall.CLONE_NEWIPC,
		}
	}

	// Rootless: create a user namespace and establish UID/GID mappings, then
	// unshare the mount namespace before exec so mount operations are permitted.
	return &syscall.SysProcAttr{
		Cloneflags: syscall.CLONE_NEWUSER |
			syscall.CLONE_NEWPID |
			syscall.CLONE_NEWNET |
			syscall.CLONE_NEWUTS |
			syscall.CLONE_NEWIPC,
		Unshareflags: syscall.CLONE_NEWNS,
		UidMappings: []syscall.SysProcIDMap{
			{ContainerID: 0, HostID: os.Getuid(), Size: 1},
		},
		GidMappings: []syscall.SysProcIDMap{
			{ContainerID: 0, HostID: os.Getgid(), Size: 1},
		},
		GidMappingsEnableSetgroups: false,
	}
}

func (s *Sandbox) tryUseCgroupFD(cmd *exec.Cmd) (*os.File, bool) {
	if s.cg == nil || cmd.SysProcAttr == nil {
		return nil, false
	}

	f, err := os.Open(s.cg.Path())
	if err != nil {
		return nil, false
	}

	cmd.SysProcAttr.UseCgroupFD = true
	cmd.SysProcAttr.CgroupFD = int(f.Fd())
	return f, true
}

func (s *Sandbox) maybeAddToCgroup(cmd *exec.Cmd, startedInCgroup bool) error {
	if s.cg == nil || startedInCgroup {
		return nil
	}

	if cmd.Process == nil {
		return errors.New("child process is nil")
	}
	return s.cg.AddProcess(cmd.Process.Pid)
}

func startWallTimer(cmd *exec.Cmd, limit time.Duration) *time.Timer {
	return time.AfterFunc(limit, func() {
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
	})
}

type memoryPoller struct {
	cg   *cgroup.Cgroup
	stop chan struct{}
	done chan struct{}
	peak int64
}

func newMemoryPoller(cg *cgroup.Cgroup) *memoryPoller {
	if cg == nil {
		return nil
	}

	p := &memoryPoller{
		cg:   cg,
		stop: make(chan struct{}),
		done: make(chan struct{}),
	}
	go p.run()
	return p
}

func (p *memoryPoller) run() {
	defer close(p.done)

	ticker := time.NewTicker(time.Millisecond)
	defer ticker.Stop()

	for {
		select {
		case <-p.stop:
			p.sample()
			return
		case <-ticker.C:
			p.sample()
		}
	}
}

func (p *memoryPoller) sample() {
	current, err := p.cg.GetMemoryCurrent()
	if err != nil || current <= p.peak {
		return
	}
	p.peak = current
}

func (p *memoryPoller) Stop() int64 {
	close(p.stop)
	<-p.done
	return p.peak
}

func rusageFromProcessState(ps *os.ProcessState) *syscall.Rusage {
	if ps == nil {
		return nil
	}
	rusage, _ := ps.SysUsage().(*syscall.Rusage)
	return rusage
}

func cpuUsageFromRusage(rusage *syscall.Rusage) int64 {
	if rusage == nil {
		return 0
	}
	return (rusage.Utime.Sec+rusage.Stime.Sec)*microsecondsPerSecond + rusage.Utime.Usec + rusage.Stime.Usec
}

func memoryPeakFromRusage(rusage *syscall.Rusage) int64 {
	if rusage == nil {
		return 0
	}
	// Maxrss is in KiB on Linux.
	return rusage.Maxrss * bytesPerKiB
}

func (s *Sandbox) applyLimitsAndTimeouts(oomKilled bool, wallTime time.Duration, cpuUsageUS int64) bool {
	if oomKilled {
		s.result.SetMemoryLimitExceeded()
		return true
	}
	if wallTime.Milliseconds() >= s.cfg.WallTimeLimit {
		s.result.SetTimeLimitExceeded()
		return true
	}
	if cpuUsageUS/microsecondsPerMilliSec >= s.cfg.TimeLimit {
		s.result.SetTimeLimitExceeded()
		return true
	}
	return false
}

func (s *Sandbox) applyWaitError(err error, memoryPeakCgroup int64) {
	exitErr := &exec.ExitError{}
	if !errors.As(err, &exitErr) {
		s.result.SetRuntimeError(1)
		return
	}

	status, ok := exitErr.Sys().(syscall.WaitStatus)
	if !ok {
		s.result.SetRuntimeError(1)
		return
	}

	if !status.Signaled() {
		s.result.SetRuntimeError(status.ExitStatus())
		return
	}

	sig := int(status.Signal())
	switch sig {
	case int(syscall.SIGKILL):
		if memoryPeakCgroup >= s.cfg.MemoryLimitBytes() {
			s.result.SetMemoryLimitExceeded()
			return
		}
		s.result.SetTimeLimitExceeded()
	case int(syscall.SIGXCPU):
		s.result.SetTimeLimitExceeded()
	case int(syscall.SIGXFSZ):
		s.result.SetOutputLimitExceeded()
	default:
		s.result.SetSignaled(sig)
	}
}

func (s *Sandbox) copyOutputBack() {
	if s.cfg.OutputFile == "" {
		return
	}

	srcOutput := filepath.Join(s.rootDir, "work", "output.txt")
	if _, err := os.Stat(srcOutput); err != nil {
		return
	}

	if err := copyFile(srcOutput, s.cfg.OutputFile); err != nil {
		_, _ = fmt.Fprintf(os.Stderr, "failed to copy output file: %v\n", err)
	}
}

// cleanup removes the temporary root directory.
func (s *Sandbox) cleanup() {
	if s.rootDir == "" {
		return
	}

	// Remove directory (tmpfs is mounted in child's mount namespace, so no unmount needed here)
	_ = os.RemoveAll(s.rootDir)
}

// getChildConfig returns the configuration for the child process.
func (s *Sandbox) getChildConfig(syncReadyFD, syncGoFD int) *ChildConfig {
	cgroupPath := ""
	if s.cg != nil {
		cgroupPath = s.cg.Path()
	}

	return &ChildConfig{
		RootDir:       s.rootDir,
		WorkDir:       "/work",
		Binary:        "/work/program",
		InputFile:     "/work/input.txt",
		OutputFile:    "/work/output.txt",
		SyncReadyFD:   syncReadyFD,
		SyncGoFD:      syncGoFD,
		TimeLimit:     s.cfg.TimeLimitSeconds(),
		MemoryLimit:   s.cfg.MemoryLimitBytes(),
		DiskLimit:     s.cfg.DiskLimitBytes(),
		StackLimit:    s.cfg.StackLimitBytes(),
		OpenFiles:     int64(s.cfg.OpenFiles),
		MaxProcesses:  int64(s.cfg.MaxProcesses),
		CgroupPath:    cgroupPath,
		HasInputFile:  s.cfg.InputFile != "",
		HasOutputFile: s.cfg.OutputFile != "",
		NoSeccomp:     s.cfg.NoSeccomp,
		DropUID:       s.dropUID,
		RunUID:        s.runUID,
		RunGID:        s.runGID,
		DisableTHP:    s.cfg.DisableTHP,
		DisableASLR:   s.cfg.DisableASLR,
	}
}

// copyFile copies a file from src to dst.
func copyFile(src, dst string) error {
	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	dstFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	if _, copyErr := io.Copy(dstFile, srcFile); copyErr != nil {
		return copyErr
	}

	// Preserve permissions
	srcInfo, err := os.Stat(src)
	if err != nil {
		return err
	}
	return os.Chmod(dst, srcInfo.Mode())
}

// parseLddOutput parses the output of ldd command.
func parseLddOutput(output string) []string {
	var libs []string

	// Split by newlines
	for _, line := range splitLines(output) {
		// Parse lines like: libfoo.so => /path/to/libfoo.so (0x...)
		// or: /lib64/ld-linux-x86-64.so.2 (0x...)
		parts := splitWhitespace(line)
		for _, part := range parts {
			if len(part) > 0 && part[0] == '/' {
				// Check if it's a valid path (not an address)
				if _, err := os.Stat(part); err == nil {
					libs = append(libs, part)
				}
			}
		}
	}

	return libs
}

func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := range len(s) {
		if s[i] == '\n' {
			lines = append(lines, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}

func splitWhitespace(s string) []string {
	var parts []string
	start := -1
	for i := range len(s) {
		if s[i] == ' ' || s[i] == '\t' {
			if start >= 0 {
				parts = append(parts, s[start:i])
				start = -1
			}
		} else {
			if start < 0 {
				start = i
			}
		}
	}
	if start >= 0 {
		parts = append(parts, s[start:])
	}
	return parts
}
