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
                    .value_name("LANGUAGE")
                    .required(true),
            )
            .arg(
                Arg::new("input")
                    .short('i')
                    .long("input")
                    .help("input file")
                    .value_name("FILE")
                    .required(true),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .help("output file")
                    .value_name("FILE")
                    .required(true),
            )
            .arg(
                Arg::new("time_limit")
                    .long("time")
                    .help("compile time limit in second")
                    .value_name("SECONDS")
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
                    .value_name("LANGUAGE")
                    .required(true),
            )
            .arg(
                Arg::new("file")
                    .short('f')
                    .long("file")
                    .help("executable file")
                    .value_name("FILE")
                    .required(true),
            )
            .arg(
                Arg::new("input")
                    .short('i')
                    .long("input")
                    .help("test case input file")
                    .value_name("FILE")
                    .required(false),
            )
            .arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .help("test case output file")
                    .value_name("FILE")
                    .required(false),
            )
            .arg(
                Arg::new("answer")
                    .short('a')
                    .long("answer")
                    .help("test case answer file")
                    .value_name("FILE")
                    .required(false),
            )
            .arg(
                Arg::new("memory_limit")
                    .long("memory")
                    .help("memory limit in kilobytes")
                    .value_name("KB")
                    .required(true),
            )
            .arg(
                Arg::new("time_limit")
                    .long("time")
                    .help("runtime limit in second")
                    .value_name("MILLISECONDS")
                    .required(true),
            )
            .arg(
                Arg::new("env")
                    .long("env")
                    .help("environment variables")
                    .value_name("KEY=VALUE")
                    .action(clap::ArgAction::Append)
                    .required(false),
            )
            .arg(
                Arg::new("workdir")
                    .long("workdir")
                    .help("working directory")
                    .value_name("DIR")
                    .required(false),
            )
            .arg(
                Arg::new("rootdir")
                    .long("rootdir")
                    .help("root directory")
                    .value_name("DIR")
                    .required(false),
            ),
    )
}

pub fn init() -> Command {
    let app = Command::new("Sandbox").version("0.0.1");
    let app = add_build_command(app);
    add_run_command(app)
}
