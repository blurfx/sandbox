use std::{
    fs::File,
    io::{BufRead, BufReader},
};

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

fn to_bytes(vec: Vec<String>) -> Vec<u8> {
    vec.into_iter().map(|s| s.into_bytes()).flatten().collect()
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

    let output_path = option.output_path.unwrap();
    let answer_path = option.answer_path.unwrap();
    let result = diff(&output_path, &answer_path);

    JudgeResult { result }
}

pub fn diff(output_path: &str, answer_path: &str) -> ResultKind {
    let output_file = File::open(output_path).unwrap();
    let answer_file = File::open(answer_path).unwrap();

    let output_reader = BufReader::new(output_file);
    let answer_reader = BufReader::new(answer_file);

    let output_lines: Vec<String> = output_reader
        .lines()
        .map(|l| l.expect("failed to parse line"))
        .collect();
    let answer_lines: Vec<String> = answer_reader
        .lines()
        .map(|l| l.expect("failed to parse line"))
        .collect();

    let output_lines = to_bytes(trim_last_newline(output_lines));
    let answer_lines = to_bytes(trim_last_newline(answer_lines));

    if output_lines.len() != answer_lines.len() {
        return ResultKind::WrongAnswer;
    }

    let mut i = 0;
    while i < output_lines.len() {
        if output_lines[i] != answer_lines[i] {
            return ResultKind::WrongAnswer;
        }
        i += 1;
    }
    return ResultKind::Accepted;
}
