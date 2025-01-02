use anyhow::Result;
use git::Branch;

mod cli;
mod git;

fn run() -> Result<i32> {
    clap_complete::env::CompleteEnv::with_factory(cli::clap).complete();

    let matches = cli::clap().get_matches();

    match matches.subcommand() {
        Some(("add", matches)) => {
            handle_add(matches)?;
        }
        Some(("show", matches)) => {
            handle_show(matches)?;
        }
        Some((&_, _)) => unreachable!(),
        None => {}
    };

    Ok(0)
}

fn handle_add(matches: &clap::ArgMatches) -> Result<()> {
    let deps: Vec<&String> = matches.get_many("dependency").unwrap().collect();
    let repo = git::Repo::new();
    let mut current_branch = repo.branch_current()?;
    for dep in deps {
        println!(
            "adding dependency {dep} to branch {}",
            current_branch.name()
        );
        // TODO: check if dep exists
        current_branch.state.deps.push(dep.clone());
    }
    current_branch.save_state()?;

    Ok(())
}

fn handle_show(matches: &clap::ArgMatches) -> Result<()> {
    let _ = matches;
    let repo = git::Repo::new();

    let current_branch = repo.branch_current()?;
    let default_branch = Branch::new("main", &repo);

    println!("git dir: {}", repo.git_dir());
    println!("current branch: {}", current_branch.name());
    println!("          deps: {}", current_branch.state.deps.join(", "));
    println!("default branch: {}", default_branch.name());

    Ok(())
}

fn main() {
    let result = run();
    match result {
        Err(e) => {
            eprintln!("regit: error: {e:#}");
            std::process::exit(1);
        }
        Ok(code) => std::process::exit(code),
    };
}
