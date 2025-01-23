use clap::{crate_version, Arg, ArgAction, Command};
//use clap_complete::engine::{ArgValueCandidates, SubcommandCandidates};

pub fn clap() -> clap::Command {
    Command::new("giddy")
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
                .help("do not print giddy messages")
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
        .subcommand(
            Command::new("del")
                .about("remove a dependency from this branch")
                .arg(
                    Arg::new("dependency")
                        .required(true)
                        .help("branch to remove from the dependencies of this branch")
                        .num_args(1..),
                ),
        )
        .subcommand(
            Command::new("new")
                .about("add a new branch based on the current branch")
                .arg(Arg::new("name").help("name of the new branch").num_args(1)),
        )
        .subcommand(
            Command::new("show")
                .about("show git branch dependency status")
                .arg(
                    Arg::new("tree")
                        .help("show dependencies in tree form")
                        .short('t')
                        .long("tree")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("update")
                .about("rebase git branch on it's dependencies")
                .arg(
                    Arg::new("recursive")
                        .help("also update dependencies")
                        .short('r')
                        .long("recursive")
                        .action(ArgAction::SetTrue),
                ),
        )
}
