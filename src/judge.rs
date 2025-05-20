use std::fs::read_to_string;

use crate::executor::ResourceUsage;

pub struct JudgeOption {
    pub memory_limit: u64,
    pub time_limit: u64,
    pub output_path: Option<String>,
    pub answer_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ResultKind {
    Accepted,
    WrongAnswer,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    RuntimeError,
    ServerError,
}

#[derive(Debug, Clone)]
pub struct JudgeResult {
    pub result: ResultKind,
}

fn trim_last_newline(mut vec: Vec<String>) -> Vec<String> {
    if vec.last() == Some(&"\n".to_string()) {
        vec.pop();
    }
    vec
}

pub fn judge(exit_code: i32, rusage: ResourceUsage, option: JudgeOption) -> JudgeResult {
    if rusage.user_time.as_millis() as u64 > (option.time_limit * 1000) {
        return JudgeResult {
            result: ResultKind::TimeLimitExceeded,
        };
    }

    if rusage.memory > (option.memory_limit / 1024) {
        return JudgeResult {
            result: ResultKind::MemoryLimitExceeded,
        };
    }

    if exit_code != 0 {
        return JudgeResult {
            result: ResultKind::RuntimeError,
        };
    }

    if option.output_path.is_none() {
        return JudgeResult {
            result: ResultKind::Accepted,
        };
    }

    let output_path = option.output_path.unwrap();
    let answer_path = option.answer_path.unwrap();
    let result = diff(&output_path, &answer_path);

    JudgeResult { result }
}

pub fn diff(output_path: &str, answer_path: &str) -> ResultKind {
    let output = read_to_string(output_path);
    let answer = read_to_string(answer_path);
    match (output, answer) {
        (Ok(output), Ok(answer)) => {
            let output_lines: Vec<String> =
                trim_last_newline(output.lines().map(|l| l.to_string()).collect());
            let answer_lines: Vec<String> =
                trim_last_newline(answer.lines().map(|l| l.to_string()).collect());

            if output_lines.len() != answer_lines.len() {
                return ResultKind::WrongAnswer;
            }

            for i in 0..output_lines.len() {
                if output_lines[i] != answer_lines[i] {
                    return ResultKind::WrongAnswer;
                }
            }
            ResultKind::Accepted
        }
        _ => ResultKind::ServerError,
    }
}
