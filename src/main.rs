use anyhow::{anyhow, Context, Result};
use itertools::Itertools;

mod cli;
mod git;
mod graph;

fn run() -> Result<i32> {
    clap_complete::env::CompleteEnv::with_factory(cli::clap).complete();

    let matches = cli::clap().get_matches();

    match matches.subcommand() {
        Some(("add", matches)) => {
            handle_add(matches)?;
        }
        Some(("del", matches)) => {
            handle_del(matches)?;
        }
        Some(("new", matches)) => {
            handle_new(matches)?;
        }
        Some(("show", matches)) => {
            handle_show(matches)?;
        }
        Some(("update", matches)) => {
            handle_update(matches)?;
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
    let previous_deps = current_branch.state.deps.clone();
    let mut graph = repo.graph()?;
    for dep in deps {
        if previous_deps.contains(dep) {
            println!(
                "branch `{}` already depends on `{dep}`",
                current_branch.name()
            );
            continue;
        }
        println!(
            "adding dependency `{dep}` to branch `{}`",
            current_branch.name()
        );
        graph.try_add_dep(current_branch.name(), dep)?;
        current_branch.state.deps.insert(dep.clone());
    }
    current_branch.save_state()?;

    Ok(())
}

fn handle_del(matches: &clap::ArgMatches) -> Result<()> {
    let deps: Vec<&String> = matches.get_many("dependency").unwrap().collect();
    let repo = git::Repo::new();
    let mut current_branch = repo.branch_current()?;
    for dep in deps {
        println!(
            "removing dependency `{dep}` from branch `{}`",
            current_branch.name()
        );
        let did_remove = current_branch.state.deps.shift_remove(dep);
        if !did_remove {
            println!(
                "warning: dependency `{dep}` was not a dependency of branch `{}`!",
                current_branch.name()
            );
        }
    }
    current_branch.save_state()?;

    Ok(())
}

fn handle_new(matches: &clap::ArgMatches) -> Result<()> {
    let name = matches.get_one("name");
    let repo = git::Repo::new();
    let current_branch = repo.branch_current()?;

    let name = name.cloned().unwrap_or_else(|| {
        let suffix = format!("{:x}", rand::random::<u64>());
        format!("{}-{}", current_branch.name(), suffix)
    });

    println!("giddy: creating new branch `{name}`");
    let mut new_branch = repo.branch_create(&name)?;

    println!(
        "giddy: adding `{}` as dependency of `{name}`",
        current_branch.name(),
    );
    new_branch.state.deps.insert(current_branch.name().clone());
    new_branch
        .save_state()
        .with_context(|| anyhow!("saving state for branch `{}`", new_branch.name()))?;

    Ok(())
}

fn handle_show(matches: &clap::ArgMatches) -> Result<()> {
    let _ = matches;
    let repo = git::Repo::new();

    let current_branch = repo.branch_current()?;
    let default_branch = repo.branch_default()?;
    let base_branch = current_branch.state.base.as_ref();

    println!("git dir: {}", repo.git_dir());
    println!(
        "current branch: {} (parent: {}{})",
        current_branch.name(),
        base_branch.unwrap_or(&String::from("none")),
        if current_branch.merged().is_ok_and(|merged| merged) {
            " (merged)"
        } else if current_branch.equal(default_branch.name())? {
            " (equal)"
        } else if current_branch.state.dirty {
            " (dirty)"
        } else {
            ""
        }
    );

    println!("  needs update: {}", current_branch.needs_update()?);
    if !current_branch.state.deps.is_empty() {
        println!(
            "          deps: {}",
            current_branch.state.deps.iter().join(", ")
        );
    }

    println!("default branch: {}", default_branch.name());

    if matches.get_flag("tree") {
        let graph = repo.graph()?;
        use ptree::graph::print_graph;

        let graph = graph.reversed();
        let branch_id = *graph.branch_id(default_branch.name())?;
        let graph = graph.graph.into_inner();

        print_graph(&graph, branch_id)?;
    }

    Ok(())
}

fn handle_update(matches: &clap::ArgMatches) -> Result<()> {
    let recursive = matches.get_flag("recursive");
    let repo = git::Repo::new();
    let current_branch = repo.branch_current()?;

    if recursive {
        use git::Branch;
        use petgraph::visit::DfsPostOrder;

        let graph = repo.graph()?;

        let mut dfs = DfsPostOrder::new(&graph.graph, *graph.branch_id(current_branch.name())?);
        while let Some(nx) = dfs.next(&graph.graph) {
            let branch_name = &graph.graph[nx];
            let mut branch = Branch::new(branch_name, &repo)?;
            branch.update()?
        }
    } else {
        let mut current_branch = repo.branch_current()?;
        current_branch.update()?;
    }

    Ok(())
}

fn main() {
    let result = run();
    match result {
        Err(e) => {
            eprintln!("giddy: error: {e:#}");
            std::process::exit(1);
        }
        Ok(code) => std::process::exit(code),
    };
}
