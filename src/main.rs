extern crate clap;
extern crate nix;

mod cli;
mod compile;
mod executor;

use compile::{compile, CompileOption};

fn main() {
    let matches = cli::init().get_matches();

    match matches.subcommand() {
        ("build", Some(sub_matches)) => {
            let language = sub_matches.value_of("language").unwrap().to_string();
            let input_path = sub_matches.value_of("input").unwrap().to_string();
            let output_path = sub_matches.value_of("output").unwrap().to_string();
            let option = CompileOption {
                language,
                input_path,
                output_path,
            };

            let succeed = compile(option);
            if succeed == 0{
                println!("ok");
            } else {
                println!("no");
            }
        }
        ("run", Some(_sub_matches)) => {
            // TODO: implement execute command
        }
        _ => {
            panic!("no subcommand given")
        }
    }
}
