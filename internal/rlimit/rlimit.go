package rlimit

import (
	"errors"
	"fmt"
	"syscall"

	"golang.org/x/sys/unix"
)

// Limits holds resource limit configurations.
type Limits struct {
	CPUTime   int64 // CPU time in seconds
	Memory    int64 // Address space in bytes
	FileSize  int64 // Maximum file size in bytes
	Stack     int64 // Stack size in bytes
	OpenFiles int64 // Maximum number of open files
	Processes int64 // Maximum number of processes
	Core      int64 // Core dump size (usually 0)
}

// Apply applies all resource limits.
func Apply(l *Limits) error {
	// RLIMIT_CPU: CPU time limit in seconds
	if l.CPUTime > 0 {
		soft := uint64(l.CPUTime)
		hard := soft + 1
		if err := setLimit(syscall.RLIMIT_CPU, soft, hard); err != nil {
			return fmt.Errorf("failed to set RLIMIT_CPU: %w", err)
		}
	}

	// RLIMIT_AS: Address space (virtual memory) limit
	if l.Memory > 0 {
		if err := setLimit(syscall.RLIMIT_AS, uint64(l.Memory), uint64(l.Memory)); err != nil {
			return fmt.Errorf("failed to set RLIMIT_AS: %w", err)
		}
	}

	// RLIMIT_FSIZE: Maximum file size
	if l.FileSize > 0 {
		if err := setLimit(syscall.RLIMIT_FSIZE, uint64(l.FileSize), uint64(l.FileSize)); err != nil {
			return fmt.Errorf("failed to set RLIMIT_FSIZE: %w", err)
		}
	}

	// RLIMIT_STACK: Stack size
	if l.Stack > 0 {
		if err := setLimit(syscall.RLIMIT_STACK, uint64(l.Stack), uint64(l.Stack)); err != nil {
			return fmt.Errorf("failed to set RLIMIT_STACK: %w", err)
		}
	}

	// RLIMIT_NOFILE: Number of open files
	if l.OpenFiles > 0 {
		if err := setLimit(syscall.RLIMIT_NOFILE, uint64(l.OpenFiles), uint64(l.OpenFiles)); err != nil {
			return fmt.Errorf("failed to set RLIMIT_NOFILE: %w", err)
		}
	}

	// RLIMIT_NPROC: Number of processes
	if l.Processes > 0 {
		if err := setLimit(unix.RLIMIT_NPROC, uint64(l.Processes), uint64(l.Processes)); err != nil {
			return fmt.Errorf("failed to set RLIMIT_NPROC: %w", err)
		}
	}

	// RLIMIT_CORE: Core dump size (disable core dumps)
	if l.Core < 0 {
		return errors.New("core limit must be non-negative")
	}
	core := uint64(l.Core)
	if err := setLimit(syscall.RLIMIT_CORE, core, core); err != nil {
		return fmt.Errorf("failed to set RLIMIT_CORE: %w", err)
	}

	return nil
}

// setLimit sets a resource limit.
func setLimit(resource int, soft, hard uint64) error {
	rlimit := &syscall.Rlimit{
		Cur: soft,
		Max: hard,
	}
	return syscall.Setrlimit(resource, rlimit)
}

// GetLimit gets the current limit for a resource.
func GetLimit(resource int) (uint64, uint64, error) {
	var rlimit syscall.Rlimit
	if err := syscall.Getrlimit(resource, &rlimit); err != nil {
		return 0, 0, err
	}
	return rlimit.Cur, rlimit.Max, nil
}
