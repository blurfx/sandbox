package config

import (
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"
)

const (
	defaultTimeLimitMS     int64 = 1000
	defaultWallTimeLimitMS int64 = 2000
	defaultMemoryLimitMB   int64 = 256
	defaultDiskLimitMB     int64 = 64
	defaultStackLimitKB    int64 = 8192

	defaultOpenFiles    int = 64
	defaultMaxProcesses int = 1

	bytesPerKiB           int64 = 1024
	bytesPerMiB                 = bytesPerKiB * bytesPerKiB
	millisecondsPerSecond int64 = 1000
)

// Config holds all sandbox configuration.
type Config struct {
	// Required
	Binary string // Path to the binary to execute

	// Resource limits
	TimeLimit     int64 // CPU time limit in milliseconds
	WallTimeLimit int64 // Wall time limit in milliseconds
	MemoryLimit   int64 // Memory limit in MB
	DiskLimit     int64 // Disk limit in MB
	StackLimit    int64 // Stack size limit in KB
	OpenFiles     int   // Maximum number of open files
	MaxProcesses  int   // Maximum number of processes

	// I/O
	InputFile  string // Path to input file (stdin)
	OutputFile string // Path to output file (stdout)

	// Internal flags
	IsChild   bool   // Whether this is the child process
	ChildArgs string // Serialized args for child process

	// Optional features
	NoCgroup  bool // Disable cgroup
	NoSeccomp bool // Disable seccomp

	// Determinism / reporting
	MemoryMetric string // Memory reporting metric: cgroup|rss
	DisableTHP   bool   // Disable transparent huge pages for the executed program
	DisableASLR  bool   // Disable ASLR for the executed program (reduces security)
}

// DefaultConfig returns a Config with sensible defaults.
func DefaultConfig() *Config {
	return &Config{
		TimeLimit:     defaultTimeLimitMS,     // 1 second
		WallTimeLimit: defaultWallTimeLimitMS, // 2 seconds
		MemoryLimit:   defaultMemoryLimitMB,   // 256 MB
		DiskLimit:     defaultDiskLimitMB,     // 64 MB
		StackLimit:    defaultStackLimitKB,    // 8 MB
		OpenFiles:     defaultOpenFiles,       // 64 files
		MaxProcesses:  defaultMaxProcesses,    // Single process
		MemoryMetric:  "cgroup",
	}
}

// ParseFlags parses command line flags and returns a Config.
func ParseFlags() (*Config, error) {
	cfg := DefaultConfig()

	flag.StringVar(&cfg.Binary, "binary", "", "Path to the binary to execute (required)")
	flag.Int64Var(&cfg.TimeLimit, "time-limit", cfg.TimeLimit, "CPU time limit in milliseconds")
	flag.Int64Var(&cfg.WallTimeLimit, "wall-time-limit", cfg.WallTimeLimit, "Wall time limit in milliseconds")
	flag.Int64Var(&cfg.MemoryLimit, "memory-limit", cfg.MemoryLimit, "Memory limit in MB")
	flag.Int64Var(&cfg.DiskLimit, "disk-limit", cfg.DiskLimit, "Disk limit in MB")
	flag.Int64Var(&cfg.StackLimit, "stack-limit", cfg.StackLimit, "Stack size limit in KB")
	flag.IntVar(&cfg.OpenFiles, "open-files", cfg.OpenFiles, "Maximum number of open files")
	flag.IntVar(&cfg.MaxProcesses, "max-processes", cfg.MaxProcesses, "Maximum number of processes")
	flag.StringVar(&cfg.InputFile, "input", "", "Path to input file (stdin)")
	flag.StringVar(&cfg.OutputFile, "output", "", "Path to output file (stdout)")

	// Internal flags for child process
	flag.BoolVar(&cfg.IsChild, "child", false, "Internal: run as child process")
	flag.StringVar(&cfg.ChildArgs, "child-args", "", "Internal: serialized child arguments")

	// Optional features
	flag.BoolVar(&cfg.NoCgroup, "no-cgroup", false, "Disable cgroup")
	flag.BoolVar(&cfg.NoSeccomp, "no-seccomp", false, "Disable seccomp")

	// Determinism / reporting
	flag.StringVar(&cfg.MemoryMetric, "memory-metric", cfg.MemoryMetric, "Memory reporting metric: cgroup or rss")
	flag.BoolVar(
		&cfg.DisableTHP,
		"disable-thp",
		false,
		"Disable transparent huge pages for the executed program (may improve memory determinism)",
	)
	flag.BoolVar(
		&cfg.DisableASLR,
		"disable-aslr",
		false,
		"Disable ASLR for the executed program (reduces security; may improve memory determinism)",
	)

	flag.Parse()

	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	return cfg, nil
}

// Validate checks if the configuration is valid.
func (c *Config) Validate() error {
	// Skip validation for child process (it receives config differently)
	if c.IsChild {
		return nil
	}

	if err := c.validateBinary(); err != nil {
		return err
	}
	if err := c.validateLimits(); err != nil {
		return err
	}
	if err := c.normalizeMemoryMetric(); err != nil {
		return err
	}
	if err := c.validateInputFile(); err != nil {
		return err
	}
	if err := c.validateOutputFile(); err != nil {
		return err
	}

	return nil
}

func (c *Config) validateBinary() error {
	if c.Binary == "" {
		return errors.New("--binary is required")
	}

	absPath, err := filepath.Abs(c.Binary)
	if err != nil {
		return fmt.Errorf("invalid binary path: %w", err)
	}
	c.Binary = absPath

	info, err := os.Stat(c.Binary)
	if err != nil {
		return fmt.Errorf("binary not found: %w", err)
	}
	if info.IsDir() {
		return errors.New("binary path is a directory")
	}
	if info.Mode()&0111 == 0 {
		return errors.New("binary is not executable")
	}

	return nil
}

func (c *Config) validateLimits() error {
	if err := validatePositiveInt64("time-limit", c.TimeLimit); err != nil {
		return err
	}
	if err := validatePositiveInt64("wall-time-limit", c.WallTimeLimit); err != nil {
		return err
	}
	if err := validatePositiveInt64("memory-limit", c.MemoryLimit); err != nil {
		return err
	}
	if err := validatePositiveInt64("disk-limit", c.DiskLimit); err != nil {
		return err
	}
	if err := validatePositiveInt64("stack-limit", c.StackLimit); err != nil {
		return err
	}
	if err := validatePositiveInt("open-files", c.OpenFiles); err != nil {
		return err
	}
	if err := validatePositiveInt("max-processes", c.MaxProcesses); err != nil {
		return err
	}
	return nil
}

func (c *Config) normalizeMemoryMetric() error {
	switch c.MemoryMetric {
	case "", "cgroup":
		c.MemoryMetric = "cgroup"
		return nil
	case "rss":
		return nil
	default:
		return fmt.Errorf("invalid memory-metric %q (expected cgroup or rss)", c.MemoryMetric)
	}
}

func (c *Config) validateInputFile() error {
	if c.InputFile == "" {
		return nil
	}

	absInput, absErr := filepath.Abs(c.InputFile)
	if absErr != nil {
		return fmt.Errorf("invalid input file path: %w", absErr)
	}
	c.InputFile = absInput

	if _, statErr := os.Stat(c.InputFile); statErr != nil {
		return fmt.Errorf("input file not found: %w", statErr)
	}

	return nil
}

func (c *Config) validateOutputFile() error {
	if c.OutputFile == "" {
		return nil
	}

	absOutput, absErr := filepath.Abs(c.OutputFile)
	if absErr != nil {
		return fmt.Errorf("invalid output file path: %w", absErr)
	}
	c.OutputFile = absOutput

	dir := filepath.Dir(c.OutputFile)
	if _, statErr := os.Stat(dir); statErr != nil {
		return fmt.Errorf("output directory does not exist: %w", statErr)
	}

	return nil
}

func validatePositiveInt64(flagName string, value int64) error {
	if value <= 0 {
		return fmt.Errorf("%s must be positive", flagName)
	}
	return nil
}

func validatePositiveInt(flagName string, value int) error {
	if value <= 0 {
		return fmt.Errorf("%s must be positive", flagName)
	}
	return nil
}

// MemoryLimitBytes returns memory limit in bytes.
func (c *Config) MemoryLimitBytes() int64 {
	return c.MemoryLimit * bytesPerMiB
}

// DiskLimitBytes returns disk limit in bytes.
func (c *Config) DiskLimitBytes() int64 {
	return c.DiskLimit * bytesPerMiB
}

// StackLimitBytes returns stack limit in bytes.
func (c *Config) StackLimitBytes() int64 {
	return c.StackLimit * bytesPerKiB
}

// TimeLimitSeconds returns CPU time limit in seconds (rounded up).
func (c *Config) TimeLimitSeconds() int64 {
	return (c.TimeLimit + (millisecondsPerSecond - 1)) / millisecondsPerSecond
}
