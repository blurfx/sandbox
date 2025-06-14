use std::fs::read_to_string;

use crate::executor::ResourceUsage;
use crate::exit_code::ExitCode;
use serde::Serialize;

pub struct JudgeOption {
    pub memory_limit: u64,
    pub time_limit: u64,
    pub output_path: Option<String>,
    pub answer_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum JudgeResultType {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "wrong_answer")]
    WrongAnswer,
    #[serde(rename = "time_limit_exceeded")]
    TimeLimitExceeded,
    #[serde(rename = "memory_limit_exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "runtime_error")]
    RuntimeError,
    #[serde(rename = "system_error")]
    SystemError,
}

#[derive(Debug, Clone)]
pub struct JudgeResult {
    pub result: JudgeResultType,
    pub memory: u64,
    pub runtime: u64,
}

fn trim_last_newline(mut vec: Vec<String>) -> Vec<String> {
    if vec.last() == Some(&"\n".to_string()) {
        vec.pop();
    }
    vec.iter().map(|s| s.trim_end().to_string()).collect()
}

pub fn judge(exit_code: i32, rusage: ResourceUsage, option: JudgeOption) -> JudgeResult {
    let runtime = (rusage.user_time.as_millis() + rusage.cpu_time.as_millis()) as u64;

    if exit_code == ExitCode::MEMORY_LIMIT_EXCEEDED as i32 {
        return JudgeResult {
            result: JudgeResultType::MemoryLimitExceeded,
            memory: rusage.memory,
            runtime: runtime,
        };
    }

    if runtime > option.time_limit {
        return JudgeResult {
            result: JudgeResultType::TimeLimitExceeded,
            memory: rusage.memory,
            runtime: runtime,
        };
    }

    if rusage.memory > option.memory_limit {
        return JudgeResult {
            result: JudgeResultType::MemoryLimitExceeded,
            memory: rusage.memory,
            runtime: runtime,
        };
    }

    if exit_code != 0 {
        return JudgeResult {
            result: JudgeResultType::RuntimeError,
            memory: rusage.memory,
            runtime: runtime,
        };
    }

    if option.output_path.is_none() {
        return JudgeResult {
            result: JudgeResultType::Accepted,
            memory: rusage.memory,
            runtime: runtime,
        };
    }

    let output_path = option.output_path.unwrap();
    let answer_path = option.answer_path.unwrap();
    let result = diff(&output_path, &answer_path);

    JudgeResult {
        result,
        memory: rusage.memory,
        runtime: runtime,
    }
}

pub fn diff(output_path: &str, answer_path: &str) -> JudgeResultType {
    let output = read_to_string(output_path);
    let answer = read_to_string(answer_path);
    match (output, answer) {
        (Ok(output), Ok(answer)) => {
            let output_lines: Vec<String> =
                trim_last_newline(output.lines().map(|l| l.to_string()).collect());
            let answer_lines: Vec<String> =
                trim_last_newline(answer.lines().map(|l| l.to_string()).collect());

            if output_lines.len() != answer_lines.len() {
                return JudgeResultType::WrongAnswer;
            }

            for i in 0..output_lines.len() {
                if output_lines[i] != answer_lines[i] {
                    return JudgeResultType::WrongAnswer;
                }
            }
            JudgeResultType::Accepted
        }
        _ => JudgeResultType::SystemError,
    }
}
