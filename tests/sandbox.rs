use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum JudgeResult {
    Accepted,
    WrongAnswer,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    RuntimeError,
    SystemError,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CommandResult {
    Compile {
        exit_code: i32,
        #[allow(dead_code)]
        memory: u64,
        #[allow(dead_code)]
        runtime: u64,
    },
    Run {
        exit_code: i32,
        memory: u64,
        runtime: u64,
        result: JudgeResult,
    },
}

#[derive(Debug)]
struct RunOutcome {
    exit_code: i32,
    memory: u64,
    runtime: u64,
    result: JudgeResult,
}

const HELLO_C: &str = r#"
#include <stdio.h>
int main(void) {
    printf("hello world\n");
    return 0;
}
"#;

const LARGE_ARRAY_C: &str = r#"
#include <stdint.h>
#include <stdlib.h>

int main(void) {
    size_t n = 100000000;
    int *arr = (int *)malloc(sizeof(int) * n);
    if (!arr) {
        return 2;
    }

    for (size_t i = 0; i < n; i++) {
        arr[i] = (int)i;
    }

    volatile int sink = arr[n - 1];
    free(arr);
    (void)sink;
    return 0;
}
"#;

const LOOP_C: &str = r#"
#include <stdint.h>

int main(void) {
    volatile uint64_t acc = 1;
    for (uint64_t i = 0; i < 1000000000ULL; i++) {
        acc = (acc ^ i) + (acc << 1);
        acc ^= (acc >> 3);
    }
    return (int)(acc & 0xFF);
}
"#;

const MEMORY_STRESS_C: &str = r#"
#include <stdint.h>
#include <stdlib.h>

int main(void) {
    size_t guard_len = (6 * 1024 * 1024) / sizeof(int);
    int *guard = (int *)malloc(sizeof(int) * guard_len);
    if (!guard) {
        return 3;
    }

    for (size_t i = 0; i < guard_len; i++) {
        guard[i] = (int)i;
    }

    size_t n = 100000000;
    int *arr = (int *)malloc(sizeof(int) * n);
    for (size_t i = 0; i < n; i++) {
        arr[i] = (int)i;
    }

    volatile int sink = guard[guard_len - 1] + arr[0];
    free(guard);
    free(arr);
    return sink;
}
"#;

fn sandbox_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sandbox"))
}

fn run_command(args: &[String], workdir: &Path) -> CommandResult {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.yaml");
    let mut full_args = vec![
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
    ];
    full_args.extend_from_slice(args);

    let output = Command::new(sandbox_bin())
        .args(&full_args)
        .current_dir(workdir)
        .output()
        .expect("failed to run sandbox");

    assert!(
        output.status.success(),
        "sandbox exited with {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("failed to parse sandbox output: {err}, stdout: {stdout}"))
}

fn write_source(dir: &Path, filename: &str, contents: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::write(&path, contents).expect("failed to write source file");
    path
}

fn compile_c(dir: &Path, source: &str, output_name: &str, time_limit_ms: u64) -> PathBuf {
    let source_path = write_source(dir, "main.c", source);
    let output_path = dir.join(output_name);
    let args = vec![
        "build".to_string(),
        "-l".to_string(),
        "c".to_string(),
        "-i".to_string(),
        source_path.to_string_lossy().into_owned(),
        "-o".to_string(),
        output_path.to_string_lossy().into_owned(),
        "--time".to_string(),
        time_limit_ms.to_string(),
    ];

    let result = run_command(&args, dir);
    match result {
        CommandResult::Compile { exit_code, .. } => {
            assert_eq!(exit_code, 0, "compile failed with exit code {exit_code}");
        }
        _ => panic!("expected compile result"),
    }

    output_path
}

fn run_binary(dir: &Path, binary: &Path, memory_limit_kb: u64, time_limit_ms: u64) -> RunOutcome {
    let args = vec![
        "run".to_string(),
        "-l".to_string(),
        "c".to_string(),
        "-f".to_string(),
        binary.to_string_lossy().into_owned(),
        "--memory".to_string(),
        memory_limit_kb.to_string(),
        "--time".to_string(),
        time_limit_ms.to_string(),
    ];

    let result = run_command(&args, dir);
    match result {
        CommandResult::Run {
            exit_code,
            memory,
            runtime,
            result,
        } => RunOutcome {
            exit_code,
            memory,
            runtime,
            result,
        },
        _ => panic!("expected run result"),
    }
}

#[test]
fn builds_simple_c_program() {
    let dir = tempdir().expect("failed to create temp dir");
    for i in 0..10 {
        let binary = compile_c(dir.path(), HELLO_C, &format!("hello_{i}.out"), 5000);
        assert!(
            binary.exists(),
            "compile iteration {i}: binary not found at {}",
            binary.display()
        );
    }
}

#[test]
fn large_array_runs_with_stable_resources() {
    let dir = tempdir().expect("failed to create temp dir");
    let binary = compile_c(dir.path(), LARGE_ARRAY_C, "array.out", 20000);

    let _warmup = run_binary(dir.path(), &binary, 500_000, 20000);

    let mut runtime_diffs = Vec::new();
    for attempt in 0..10 {
        let first = run_binary(dir.path(), &binary, 500_000, 20000);
        let second = run_binary(dir.path(), &binary, 500_000, 20000);

        assert_eq!(
            first.result,
            JudgeResult::Accepted,
            "attempt {attempt} first run not accepted"
        );
        assert_eq!(
            second.result,
            JudgeResult::Accepted,
            "attempt {attempt} second run not accepted"
        );

        let memory_diff = first.memory.abs_diff(second.memory);
        assert!(
            memory_diff <= 8,
            "attempt {attempt} memory differed ({} vs {}, diff {})",
            first.memory,
            second.memory,
            memory_diff
        );

        let runtime_diff = first.runtime.abs_diff(second.runtime);
        runtime_diffs.push(runtime_diff);
    }

    runtime_diffs.sort_unstable();
    let median = runtime_diffs[runtime_diffs.len() / 2];
    assert!(
        median <= 10,
        "median runtime drift {median}ms too high; diffs: {runtime_diffs:?}"
    );
}

#[test]
fn tight_loop_is_consistently_time_limited() {
    let dir = tempdir().expect("failed to create temp dir");
    let binary = compile_c(dir.path(), LOOP_C, "loop.out", 10000);

    for attempt in 0..10 {
        let run = run_binary(dir.path(), &binary, 262_144, 1000);
        assert_eq!(
            run.result,
            JudgeResult::TimeLimitExceeded,
            "run {} did not time out (exit {}, runtime {}ms, memory {}KB)",
            attempt,
            run.exit_code,
            run.runtime,
            run.memory
        );
    }
}

#[test]
fn dynamic_allocation_hits_memory_limit() {
    let dir = tempdir().expect("failed to create temp dir");
    let binary = compile_c(dir.path(), MEMORY_STRESS_C, "memory.out", 20000);

    for attempt in 0..10 {
        let run = run_binary(dir.path(), &binary, 5120, 20000);
        assert_eq!(
            run.result,
            JudgeResult::MemoryLimitExceeded,
            "run {} did not hit memory limit (exit {}, runtime {}ms, memory {}KB)",
            attempt,
            run.exit_code,
            run.runtime,
            run.memory
        );
    }
}
