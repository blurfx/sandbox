package cgroup

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const (
	cgroupBasePath = "/sys/fs/cgroup"
)

// Cgroup handles cgroup v2 operations.
type Cgroup struct {
	name string
	path string
}

// New creates a new cgroup manager with the given name
// It tries to create the cgroup under the current process's cgroup if possible.
func New(name string) *Cgroup {
	basePath := getBasePath()
	return &Cgroup{
		name: name,
		path: filepath.Join(basePath, name),
	}
}

// getBasePath returns the base path for creating cgroups
// When running as root (uid 0), use /sys/fs/cgroup directly for full control
// Otherwise, try to use the user@.service cgroup (not scopes, which are leaf cgroups).
func getBasePath() string {
	const cgroupLineParts = 3

	// If running as root, use the cgroup root directly
	if os.Getuid() == 0 {
		return cgroupBasePath
	}

	// For non-root, use the user@.service cgroup
	// Scopes (like tmux-spawn-*.scope) are leaf cgroups and don't delegate controllers
	data, err := os.ReadFile("/proc/self/cgroup")
	if err != nil {
		return cgroupBasePath
	}

	// Parse cgroup v2 entry (format: "0::/path")
	for line := range strings.SplitSeq(string(data), "\n") {
		parts := strings.SplitN(line, ":", cgroupLineParts)
		if len(parts) != cgroupLineParts || parts[0] != "0" {
			continue
		}

		cgroupPath := strings.TrimSpace(parts[2])
		if cgroupPath == "" || cgroupPath == "/" {
			continue
		}

		// Extract user@.service path, removing any scope suffix
		// e.g., /user.slice/user-1001.slice/user@1001.service/tmux-spawn-*.scope
		// becomes /user.slice/user-1001.slice/user@1001.service
		if idx := strings.Index(cgroupPath, "/user@"); idx != -1 {
			// Find the end of user@UID.service.
			serviceEnd := strings.Index(cgroupPath[idx:], ".service")
			if serviceEnd != -1 {
				cgroupPath = cgroupPath[:idx+serviceEnd+len(".service")]
			}
		}

		return filepath.Join(cgroupBasePath, cgroupPath)
	}

	return cgroupBasePath
}

// Path returns the full path to the cgroup.
func (m *Cgroup) Path() string {
	return m.path
}

// Name returns the cgroup name.
func (m *Cgroup) Name() string {
	return m.name
}

// Create creates the cgroup directory and enables controllers.
func (m *Cgroup) Create() error {
	// Create the cgroup directory
	if err := os.MkdirAll(m.path, 0750); err != nil {
		return fmt.Errorf("failed to create cgroup directory: %w", err)
	}

	// Enable memory and pids controllers in parent
	// First, check what controllers are available
	parentPath := filepath.Dir(m.path)
	controllersPath := filepath.Join(parentPath, "cgroup.subtree_control")

	// Try to enable required controllers
	controllers := []string{"+memory", "+pids", "+cpu"}
	for _, ctrl := range controllers {
		// Ignore errors as some controllers might not be available
		_ = os.WriteFile(controllersPath, []byte(ctrl), 0600)
	}

	return nil
}

// SetMemoryLimit sets the memory limit in bytes.
func (m *Cgroup) SetMemoryLimit(bytes int64) error {
	path := filepath.Join(m.path, "memory.max")
	return os.WriteFile(path, []byte(strconv.FormatInt(bytes, 10)), 0600)
}

// SetMemorySwapLimit sets the swap limit (0 to disable swap).
func (m *Cgroup) SetMemorySwapLimit(bytes int64) error {
	path := filepath.Join(m.path, "memory.swap.max")
	return os.WriteFile(path, []byte(strconv.FormatInt(bytes, 10)), 0600)
}

// SetPidsLimit sets the maximum number of processes.
func (m *Cgroup) SetPidsLimit(n int) error {
	path := filepath.Join(m.path, "pids.max")
	return os.WriteFile(path, []byte(strconv.Itoa(n)), 0600)
}

// AddProcess adds a process to the cgroup.
func (m *Cgroup) AddProcess(pid int) error {
	path := filepath.Join(m.path, "cgroup.procs")
	return os.WriteFile(path, []byte(strconv.Itoa(pid)), 0600)
}

// GetMemoryPeak returns the peak memory usage in bytes.
func (m *Cgroup) GetMemoryPeak() (int64, error) {
	path := filepath.Join(m.path, "memory.peak")
	data, err := os.ReadFile(path)
	if err != nil {
		return 0, fmt.Errorf("failed to read memory.peak: %w", err)
	}

	value, err := strconv.ParseInt(strings.TrimSpace(string(data)), 10, 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse memory.peak: %w", err)
	}

	return value, nil
}

// GetMemoryCurrent returns the current memory usage in bytes.
func (m *Cgroup) GetMemoryCurrent() (int64, error) {
	path := filepath.Join(m.path, "memory.current")
	data, err := os.ReadFile(path)
	if err != nil {
		return 0, fmt.Errorf("failed to read memory.current: %w", err)
	}

	value, err := strconv.ParseInt(strings.TrimSpace(string(data)), 10, 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse memory.current: %w", err)
	}

	return value, nil
}

// GetCPUUsage returns the CPU usage in microseconds from cpu.stat.
func (m *Cgroup) GetCPUUsage() (int64, error) {
	const cpuStatFieldCountMin = 2

	path := filepath.Join(m.path, "cpu.stat")
	file, err := os.Open(path)
	if err != nil {
		return 0, fmt.Errorf("failed to open cpu.stat: %w", err)
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "usage_usec ") {
			parts := strings.Fields(line)
			if len(parts) >= cpuStatFieldCountMin {
				return strconv.ParseInt(parts[1], 10, 64)
			}
		}
	}

	if scanErr := scanner.Err(); scanErr != nil {
		return 0, fmt.Errorf("failed to read cpu.stat: %w", scanErr)
	}

	return 0, errors.New("usage_usec not found in cpu.stat")
}

// IsOOMKilled checks if any process in the cgroup was killed by OOM.
func (m *Cgroup) IsOOMKilled() (bool, error) {
	const memoryEventsFieldCountMin = 2

	path := filepath.Join(m.path, "memory.events")
	file, err := os.Open(path)
	if err != nil {
		return false, fmt.Errorf("failed to open memory.events: %w", err)
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		if strings.HasPrefix(line, "oom_kill ") {
			parts := strings.Fields(line)
			if len(parts) >= memoryEventsFieldCountMin {
				count, _ := strconv.ParseInt(parts[1], 10, 64)
				return count > 0, nil
			}
		}
	}

	return false, nil
}

// Destroy removes the cgroup.
func (m *Cgroup) Destroy() error {
	// First, kill any remaining processes
	procsPath := filepath.Join(m.path, "cgroup.procs")
	data, err := os.ReadFile(procsPath)
	if err == nil {
		scanner := bufio.NewScanner(strings.NewReader(string(data)))
		for scanner.Scan() {
			pid, parseErr := strconv.Atoi(strings.TrimSpace(scanner.Text()))
			if parseErr == nil && pid > 0 {
				// Try to kill the process
				proc, findErr := os.FindProcess(pid)
				if findErr == nil {
					_ = proc.Kill()
				}
			}
		}
	}

	// Remove the cgroup directory
	return os.Remove(m.path)
}

// Exists checks if the cgroup exists.
func (m *Cgroup) Exists() bool {
	_, err := os.Stat(m.path)
	return err == nil
}
