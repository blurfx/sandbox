extern crate clap;
extern crate core;
extern crate nix;

mod cli;
mod command;
mod config;
mod executor;
mod exit_code;
mod judge;
mod process;
pub mod seccomp;

use std::path::PathBuf;

use command::{CompileOption, RunOption, compile, run};
use process::Directory;

use crate::config::LanguageConfig;
use crate::judge::{JudgeOption, JudgeResultType, judge};
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(tag = "type")]
enum CommandResult {
    #[serde(rename = "compile")]
    Compile {
        exit_code: i32,
        memory: u64,
        runtime: u64,
    },

    #[serde(rename = "run")]
    Run {
        exit_code: i32,
        memory: u64,
        runtime: u64,
        result: JudgeResultType,
    },
}

fn main() {
    let matches = cli::init().get_matches();
    let config_path = matches
        .get_one::<String>("config")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "config.yaml".to_string());

    let language_config = match LanguageConfig::from_file(&config_path) {
        Ok(config) => config,
        Err(config::ConfigError::MissingConfig(path)) => panic!("config file not found: {}", path),
        Err(err) => panic!("failed to load language config: {}", err),
    };

    match matches.subcommand() {
        Some(("build", sub_matches)) => {
            let language = sub_matches
                .get_one::<String>("language")
                .unwrap()
                .to_string();
            let input_path = sub_matches.get_one::<String>("input").unwrap().to_string();
            let output_path = sub_matches.get_one::<String>("output").unwrap().to_string();
            let time_limit: u64 = sub_matches
                .get_one::<String>("time_limit")
                .unwrap()
                .parse()
                .unwrap();
            let option = CompileOption {
                language,
                input_path,
                output_path,
                time_limit,
            };

            let result = compile(option, &language_config);
            let runtime_micros =
                result.rusage.cpu_time.as_micros() + result.rusage.user_time.as_micros();
            let runtime = runtime_micros.div_ceil(1000) as u64;
            print!(
                "{}",
                serde_json::to_string(&CommandResult::Compile {
                    exit_code: result.exit_code,
                    memory: result.rusage.memory,
                    runtime,
                })
                .unwrap()
            );
        }
        Some(("run", sub_matches)) => {
            let language = sub_matches
                .get_one::<String>("language")
                .unwrap()
                .to_string();
            let file_path = sub_matches.get_one::<String>("file").unwrap().to_string();
            let input_path = sub_matches
                .get_one::<String>("input")
                .map(|s| s.to_string());
            let output_path = sub_matches
                .get_one::<String>("output")
                .map(|s| s.to_string());
            let answer_path = sub_matches
                .get_one::<String>("answer")
                .map(|s| s.to_string());
            let time_limit: u64 = sub_matches
                .get_one::<String>("time_limit")
                .unwrap()
                .parse()
                .unwrap();
            let memory_limit: u64 = sub_matches
                .get_one::<String>("memory_limit")
                .unwrap()
                .parse()
                .unwrap();
            let working_dir = sub_matches.get_one::<String>("workdir").map(PathBuf::from);
            let root_dir = sub_matches.get_one::<String>("rootdir").map(PathBuf::from);
            let directory = Directory {
                working_dir,
                root_dir,
            };
            let envs: Vec<_> = sub_matches
                .get_many::<String>("env")
                .unwrap_or_default()
                .collect();
            let envs = envs.iter().map(|s| s.to_string()).collect();

            let option = RunOption {
                language,
                file_path,
                input_path,
                output_path: output_path.clone(),
                time_limit,
                memory_limit,
                envs,
                directory,
            };

            let result = run(option, &language_config);
            let judge_opt = JudgeOption {
                output_path,
                answer_path,
                time_limit,
                memory_limit,
            };
            let judge_result = judge(result.exit_code, result.rusage, judge_opt);
            print!(
                "{}",
                serde_json::to_string(&CommandResult::Run {
                    exit_code: result.exit_code,
                    memory: judge_result.memory,
                    runtime: judge_result.runtime,
                    result: judge_result.result,
                })
                .unwrap()
            );
        }
        _ => {
            unreachable!("no valid subcommand given")
        }
    }
}
