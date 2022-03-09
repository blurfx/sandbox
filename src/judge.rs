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
pub enum Result {
    Accepted,
    WrongAnswer,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    OutputLimitExceeded,
    RuntimeError,
}

#[derive(Debug, Clone)]
pub struct JudgeResult {
    pub result: Result,
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
            result: Result::TimeLimitExceeded,
        };
    }

    if rusage.memory > (option.memory_limit / 1024) {
        return JudgeResult {
            result: Result::MemoryLimitExceeded,
        };
    }

    if exit_code != 0 {
        return JudgeResult {
            result: Result::RuntimeError,
        };
    }

    let output_path = option.output_path.unwrap();
    let answer_path = option.answer_path.unwrap();
    if diff(&output_path, &answer_path) {
        JudgeResult {
            result: Result::Accepted,
        }
    } else {
        JudgeResult {
            result: Result::WrongAnswer,
        }
    }
}

pub fn diff(output_path: &str, answer_path: &str) -> bool {
    let output_file = File::open(output_path).unwrap();
    let answer_file = File::open(answer_path).unwrap();

    let mut output_reader = BufReader::new(output_file);
    let mut answer_reader = BufReader::new(answer_file);

    let mut output_lines: Vec<String> = output_reader
        .lines()
        .map(|l| l.expect("failed to parse line"))
        .collect();
    let mut answer_lines: Vec<String> = answer_reader
        .lines()
        .map(|l| l.expect("failed to parse line"))
        .collect();

    output_lines = trim_last_newline(output_lines);
    answer_lines = trim_last_newline(answer_lines);

    if output_lines.len() != answer_lines.len() {
        return false;
    }

    for (output_line, answer_line) in output_lines.iter().zip(answer_lines.iter()) {
        if output_line != answer_line {
            return false;
        }
    }
    return true;
}
