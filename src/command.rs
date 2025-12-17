use std::vec::Vec;

use crate::{
    config::{CommandTemplate, LanguageConfig, LanguageKind},
    executor::{ExecuteOption, ExecuteResult, ResourceLimit, execute},
    process::Directory,
};

pub struct CompileOption {
    pub language: String,
    pub input_path: String,
    pub output_path: String,
    pub time_limit: u64,
}

pub struct RunOption {
    pub language: String,
    pub file_path: String,
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub time_limit: u64,
    pub memory_limit: u64,
    pub envs: Vec<String>,
    pub directory: Directory,
}

const INPUT_TOKEN: &str = "<INPUT>";
const OUTPUT_TOKEN: &str = "<OUTPUT>";
const TARGET_TOKEN: &str = "<TARGET>";

fn render_command(
    template: &CommandTemplate,
    replacements: &[(&str, &str)],
) -> (String, Vec<String>) {
    let mut bin = template.bin.clone();
    for (key, value) in replacements {
        bin = bin.replace(key, value);
    }

    let mut args = Vec::new();
    for arg in &template.args {
        let mut rendered = arg.clone();
        for (key, value) in replacements {
            rendered = rendered.replace(key, value);
        }
        args.push(rendered);
    }

    (bin, args)
}

pub fn compile(opt: CompileOption, config: &LanguageConfig) -> ExecuteResult {
    let definition = config
        .get_language(&opt.language)
        .expect(&format!("unsupported language: {}", opt.language));

    if definition.kind != LanguageKind::Compile {
        panic!("language {} does not support compilation", opt.language);
    }

    let compile_template = definition
        .compile
        .as_ref()
        .unwrap_or_else(|| panic!("compile command missing for {}", opt.language));

    let (bin, args) = render_command(
        compile_template,
        &[
            (INPUT_TOKEN, opt.input_path.as_str()),
            (OUTPUT_TOKEN, opt.output_path.as_str()),
        ],
    );

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    execute(
        bin.as_str(),
        arg_refs,
        ExecuteOption {
            envs: None,
            limits: Some(ResourceLimit {
                time: opt.time_limit,
                memory: 268435456, // 256mb
            }),
            input_path: None,
            output_path: None,
            directory: None,
            use_syscall: false,
        },
    )
}

pub fn run(opt: RunOption, config: &LanguageConfig) -> ExecuteResult {
    let definition = config
        .get_language(&opt.language)
        .expect(&format!("unsupported language: {}", opt.language));

    let (bin, args) = render_command(&definition.run, &[(TARGET_TOKEN, opt.file_path.as_str())]);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let rlimit = ResourceLimit {
        time: opt.time_limit,
        memory: opt.memory_limit,
    };

    let option = ExecuteOption {
        envs: Some(opt.envs.clone()),
        limits: Some(rlimit),
        input_path: opt.input_path.clone(),
        output_path: opt.output_path.clone(),
        directory: Some(opt.directory.clone()),
        use_syscall: true,
    };

    execute(bin.as_str(), arg_refs, option)
}
