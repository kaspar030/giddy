use camino::Utf8PathBuf;

use clap::{crate_version, Arg, ArgAction, Command, ValueHint};
//use clap_complete::engine::{ArgValueCandidates, SubcommandCandidates};

pub fn clap() -> clap::Command {
    Command::new("regit")
        .version(crate_version!())
        .author("Kaspar Schleiser <kaspar@schleiser.de>")
        .about("Tend your trees")
        .infer_subcommands(true)
        .arg(
            Arg::new("verbose")
                .help("be verbose (e.g., show command lines)")
                .short('v')
                .long("verbose")
                .global(true)
                .action(ArgAction::Count),
        )
        .arg(
            Arg::new("quiet")
                .help("do not print regit messages")
                .short('q')
                .long("quiet")
                .global(true)
                .action(ArgAction::Count)
                .hide(true), // (not really supported, yet)
        )
        .subcommand(
            Command::new("add")
                .about("add a dependency to this branch")
                .arg(
                    Arg::new("dependency")
                        .required(true)
                        .help("branch to add as dependency of this branch")
                        .num_args(1..),
                ),
        )
        .subcommand(Command::new("show").about("show git branch dependency status"))
}
