use clap::{Arg, Command};

fn add_build_command(app: Command) -> Command {
    app.subcommand(
        Command::new("build")
            .about("compile code")
            .arg(
                Arg::new("language")
                    .short('l')
                    .long("language")
                    .help("language to compile")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("input")
                    .short('i')
                    .long("input")
                    .help("input file")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .help("output file")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("time_limit")
                    .long("time")
                    .help("compile time limit in second")
                    .takes_value(true)
                    .required(false),
            ),
    )
}

fn add_run_command(app: Command) -> Command {
    app.subcommand(
        Command::new("run")
            .about("run binary or code within sandbox")
            .arg(
                Arg::new("language")
                    .short('l')
                    .long("language")
                    .help("language to compile")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("file")
                    .short('f')
                    .long("file")
                    .help("executable file")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("input")
                    .short('i')
                    .long("input")
                    .help("test case input file")
                    .takes_value(true)
                    .required(false),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .help("test case output file")
                    .takes_value(true)
                    .required(false),
            )
            .arg(
                Arg::new("answer")
                    .short('a')
                    .long("answer")
                    .help("test case answer file")
                    .takes_value(true)
                    .required(false),
            )
            .arg(
                Arg::new("memory_limit")
                    .long("memory")
                    .help("memory limit in bytes")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("time_limit")
                    .long("time")
                    .help("runtime limit in second")
                    .takes_value(true)
                    .required(true),
            )
            .arg(
                Arg::new("env")
                    .long("env")
                    .help("environment variables")
                    .takes_value(true)
                    .multiple_occurrences(true)
                    .required(false),
            )
            .arg(
                Arg::new("workdir")
                    .long("workdir")
                    .help("working directory")
                    .takes_value(true)
                    .required(false),
            )
            .arg(
                Arg::new("rootdir")
                    .long("rootdir")
                    .help("root directory")
                    .takes_value(true)
                    .required(false),
            ),
    )
}

pub fn init<'a>() -> Command<'a> {
    let app = Command::new("Sandbox").version("0.0.1");
    let app = add_build_command(app);
    add_run_command(app)
}
