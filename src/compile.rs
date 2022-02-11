use std::collections::HashMap;
use std::vec::Vec;

use crate::executor::execute;

pub struct CompileOption {
    pub language: String,
    pub input_path: String,
    pub output_path: String,
}

enum FlagToken {
    INPUT,
    OUTPUT
}

impl FlagToken {
    fn value(&self) -> &str {
        match *self {
            FlagToken::INPUT => "<INPUT>",
            FlagToken::OUTPUT => "<OUTPUT>",
        }
    }
}

fn get_compile_flags(language: &str) -> Option<(&str, Vec<&str>)> {
    let map: HashMap<&str, (&str, Vec<&str>)> = [
        ("cpp", ("g++", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
        ("c", ("gcc", vec![FlagToken::INPUT.value(), "-O2", "-Wall", "-lm", "-o", FlagToken::OUTPUT.value()])),
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
        return *arg;
    }).collect();

    execute(compiler, compile_args)
}