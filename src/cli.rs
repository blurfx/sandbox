
use clap::{App, Arg, SubCommand};

fn add_build_command<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    app.subcommand(SubCommand::with_name("build")
        .about("compile code within sandbox")
        .arg(Arg::with_name("language")
            .short("l")
            .long("language")
            .help("language to compile")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("input")
            .short("i")
            .long("input")
            .help("input file")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .help("output file")
            .takes_value(true)
            .required(true)
        )
    )
}

fn add_run_command<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    app.subcommand(SubCommand::with_name("run")
        .about("compile code within sandbox")
        .arg(Arg::with_name("language")
            .short("l")
            .long("language")
            .help("language to compile")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("bin")
            .short("b")
            .long("bin")
            .help("executable file")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("input")
            .short("i")
            .long("input")
            .help("input file")
            .takes_value(true)
            .required(true)
        )
    )
}

pub fn init<'a>() -> App<'a, 'a> {
    let app = App::new("Sandbox")
        .version("0.0.1");
    let app = add_build_command(app);
    add_run_command(app)
}