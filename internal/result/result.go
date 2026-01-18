package result

import (
	"encoding/json"
	"fmt"
	"os"
)

// Status represents the execution result status.
type Status string

const (
	StatusOK  Status = "OK"  // Normal exit with code 0
	StatusRE  Status = "RE"  // Runtime Error (non-zero exit)
	StatusTLE Status = "TLE" // Time Limit Exceeded
	StatusMLE Status = "MLE" // Memory Limit Exceeded
	StatusOLE Status = "OLE" // Output Limit Exceeded
	StatusSG  Status = "SG"  // Killed by signal
	StatusXX  Status = "XX"  // Internal Error
)

const (
	microsecondsPerMillisecond int64 = 1000
	bytesPerKiB                int64 = 1024
)

// Result represents the sandbox execution result.
type Result struct {
	Status   Status `json:"status"`
	ExitCode int    `json:"exit_code"`
	Signal   int    `json:"signal,omitempty"`

	// Resource usage
	CPUTimeMS  int64 `json:"cpu_time_ms"`  // CPU time in milliseconds
	WallTimeMS int64 `json:"wall_time_ms"` // Wall time in milliseconds
	MemoryKB   int64 `json:"memory_kb"`    // Peak memory in KB

	// Optional message for errors
	Message string `json:"message,omitempty"`
}

// New creates a new Result with default values.
func New() *Result {
	return &Result{
		Status:   StatusXX,
		ExitCode: -1,
	}
}

// SetOK marks the result as successful.
func (r *Result) SetOK() {
	r.Status = StatusOK
	r.ExitCode = 0
}

// SetRuntimeError marks the result as runtime error.
func (r *Result) SetRuntimeError(exitCode int) {
	r.Status = StatusRE
	r.ExitCode = exitCode
}

// SetTimeLimitExceeded marks the result as TLE.
func (r *Result) SetTimeLimitExceeded() {
	r.Status = StatusTLE
}

// SetMemoryLimitExceeded marks the result as MLE.
func (r *Result) SetMemoryLimitExceeded() {
	r.Status = StatusMLE
}

// SetOutputLimitExceeded marks the result as OLE.
func (r *Result) SetOutputLimitExceeded() {
	r.Status = StatusOLE
}

// SetSignaled marks the result as killed by signal.
func (r *Result) SetSignaled(signal int) {
	r.Status = StatusSG
	r.Signal = signal
}

// SetInternalError marks the result as internal error.
func (r *Result) SetInternalError(msg string) {
	r.Status = StatusXX
	r.Message = msg
}

// SetResourceUsage sets the resource usage metrics.
func (r *Result) SetResourceUsage(cpuTimeUS int64, wallTimeMS int64, memoryBytes int64) {
	r.CPUTimeMS = cpuTimeUS / microsecondsPerMillisecond
	r.WallTimeMS = wallTimeMS
	r.MemoryKB = memoryBytes / bytesPerKiB
}

// JSON returns the result as a JSON string.
func (r *Result) JSON() string {
	data, err := json.Marshal(r)
	if err != nil {
		return fmt.Sprintf(`{"status":"XX","message":"failed to marshal result: %s"}`, err.Error())
	}
	return string(data)
}

// JSONPretty returns the result as a pretty-printed JSON string.
func (r *Result) JSONPretty() string {
	data, err := json.MarshalIndent(r, "", "  ")
	if err != nil {
		return fmt.Sprintf(`{"status":"XX","message":"failed to marshal result: %s"}`, err.Error())
	}
	return string(data)
}

// Print outputs the result to stdout.
func (r *Result) Print() {
	_, _ = fmt.Fprintln(os.Stdout, r.JSON())
}

// PrintPretty outputs the result to stdout with pretty formatting.
func (r *Result) PrintPretty() {
	_, _ = fmt.Fprintln(os.Stdout, r.JSONPretty())
}

// Exit prints the result and exits with appropriate code.
func (r *Result) Exit() {
	r.Print()
	if r.Status == StatusOK {
		os.Exit(0)
	}
	os.Exit(1)
}
