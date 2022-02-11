use std::collections::HashMap;
use std::vec::Vec;

use crate::executor::execute;

pub struct CompileOption {
    pub language: String,
    pub input_path: String,
    pub output_path: String,
}

pub struct RunOption {
    pub language: String,
    pub binary_path: String,
}

enum FlagToken {
    INPUT,
    OUTPUT,
    BINARY,
}

impl FlagToken {
    fn value(&self) -> &str {
        match *self {
            FlagToken::INPUT => "<INPUT>",
            FlagToken::OUTPUT => "<OUTPUT>",
            FlagToken::BINARY => "<BINARY>",
        }
    }
}

fn get_compile_flags(language: &str) -> Option<(&str, Vec<&str>)> {
    let map: HashMap<&str, (&str, Vec<&str>)> = [
        ("c", ("gcc", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
        ("cpp", ("g++", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
    ].iter().cloned().collect();

    map.get(language).cloned()
}

fn get_run_flags(language: &str) -> Option<Vec<&str>> {
    let map: HashMap<&str, Vec<&str>> = [
        ("c", vec![FlagToken::BINARY.value()]),
        ("cpp", vec![FlagToken::BINARY.value()]),
        ("java", vec!["java", FlagToken::BINARY.value()]),
    ].iter().cloned().collect();

    map.get(language).cloned()
}

pub fn compile(opt: CompileOption) -> i32 {
    let compiler: &str;
    let compile_args: Vec<&str>;
    match get_compile_flags(&opt.language){
        Some(flags) => {
            compiler = flags.0;
            compile_args = flags.1;
        }
        _ => {
            panic!("unsupported language: {}", opt.language);
        }
    }

    let compile_args: Vec<&str> = compile_args.iter().map(|arg| {
        if *arg == FlagToken::INPUT.value() {
             return opt.input_path.as_str();
        } else if *arg == FlagToken::OUTPUT.value() {
            return opt.output_path.as_str();
        }
        
        *arg
    }).collect();

    execute(compiler, compile_args)
}

pub fn run(opt: RunOption) -> i32 {
    let args = match get_run_flags(&opt.language) {
        Some(args) => args,
        None => {
            panic!("unsupported language: {}", opt.language);
        }
    };

    let args: Vec<&str> = args.iter().map(|arg| {
        if *arg == FlagToken::BINARY.value() {
            return opt.binary_path.as_str();
        } 

        *arg
    }).collect();

    execute(&opt.binary_path, args)
}
