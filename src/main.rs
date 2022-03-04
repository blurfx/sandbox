extern crate clap;
extern crate core;
extern crate nix;

mod cli;
mod command;
mod executor;
mod exit_code;
mod judge;
mod process;
pub mod seccomp;

use std::path::PathBuf;

use command::{compile, run, CompileOption, RunOption};
use process::Directory;

fn main() {
    let matches = cli::init().get_matches();

    match matches.subcommand() {
        Some(("build", sub_matches)) => {
            let language = sub_matches.value_of("language").unwrap().to_string();
            let input_path = sub_matches.value_of("input").unwrap().to_string();
            let output_path = sub_matches.value_of("output").unwrap().to_string();
            let time_limit: u64 = sub_matches
                .value_of("time_limit")
                .unwrap_or("15")
                .parse()
                .unwrap();
            let option = CompileOption {
                language,
                input_path,
                output_path,
                time_limit,
            };

            let succeed = compile(option);
            if succeed == 0 {
                println!("ok");
            } else {
                println!("no");
            }
        }
        Some(("run", sub_matches)) => {
            let language = sub_matches.value_of("language").unwrap().to_string();
            let file_path = sub_matches.value_of("file").unwrap().to_string();
            let input_path = match sub_matches.value_of("input") {
                Some(input) => Some(input.to_string()),
                None => None,
            };
            let output_path = match sub_matches.value_of("output") {
                Some(output) => Some(output.to_string()),
                None => None,
            };
            let answer_path = match sub_matches.value_of("answer") {
                Some(answer) => Some(answer.to_string()),
                None => None,
            };
            let time_limit: u64 = sub_matches.value_of("time_limit").unwrap().parse().unwrap();
            let memory_limit: u64 = sub_matches
                .value_of("memory_limit")
                .unwrap()
                .parse()
                .unwrap();
            let working_dir = match sub_matches.value_of("workdir") {
                Some(path) => Some(PathBuf::from(path)),
                _ => None,
            };
            let root_dir = match sub_matches.value_of("rootdir") {
                Some(path) => Some(PathBuf::from(path)),
                _ => None,
            };
            let directory = Directory {
                working_dir,
                root_dir,
            };
            let envs: Vec<_> = sub_matches.values_of("env").unwrap_or_default().collect();
            let envs = envs.iter().map(|s| s.to_string()).collect();

            let option = RunOption {
                language,
                file_path,
                input_path,
                output_path,
                answer_path,
                time_limit,
                memory_limit,
                envs,
                directory,
            };

            let succeed = run(option);
            if succeed == 0 {
                println!("run ok");
            } else {
                println!("run fail");
            }
        }
        _ => {
            unreachable!("no valid subcommand given")
        }
    }
}
