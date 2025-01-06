use std::{
    ffi::OsStr,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use itertools::Itertools;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::graph::GraphRepo;

#[derive(Debug)]
pub struct Repo {
    git_dir: Utf8PathBuf,
}

#[derive(Debug, Clone)]
pub struct Branch<'a> {
    name: String,
    repo: &'a Repo,
    pub state: BranchState,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BranchState {
    pub deps: IndexSet<String>,
    pub pr: Option<u32>,
}

impl Repo {
    pub fn new() -> Repo {
        let git_dir = Repo::get_git_dir().unwrap();
        std::fs::create_dir_all(git_dir.join("regit")).unwrap();
        Repo { git_dir }
    }

    pub fn graph(&self) -> Result<GraphRepo> {
        GraphRepo::new(self)
    }

    pub fn git(&self) -> std::process::Command {
        let command = Command::new("git");
        //command.arg("-C").arg(&self.git_dir);
        #[expect(clippy::let_and_return)]
        command
    }

    pub fn git_dir(&self) -> &Utf8Path {
        self.git_dir.as_path()
    }

    pub fn get_git_dir() -> Result<Utf8PathBuf> {
        let res = Command::new("git")
            .arg("rev-parse")
            .arg("--absolute-git-dir")
            .output()
            .expect("failed to execute git");

        let res = String::from_utf8(res.stdout)?;
        let git_dir = Utf8PathBuf::from(res.trim());

        Ok(git_dir)
    }

    pub fn cmd_output<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self
            .git()
            .args(args)
            .output()
            .expect("failed to execute git");
        let res = String::from_utf8(output.stdout).expect("failed to convert git output into utf8");

        Ok(res)
    }

    pub fn cmd_output_vec<I, S>(&self, args: I) -> Result<Vec<String>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.cmd_output(args)?;
        Ok(output.lines().map(|line| line.trim().to_string()).collect())
    }

    pub fn cmd_check<I, S>(&self, args: I) -> Result<bool>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Ok(self.git().args(args).status()?.success())
    }

    pub fn branch_current(&self) -> Result<Branch<'_>> {
        let name = self.cmd_output(["branch", "--show-current"])?;
        let name = name.trim();

        Ok(Branch::new(name, self))
    }

    pub fn branch_names(&self) -> Result<Vec<String>> {
        self.cmd_output_vec(["branch", "--format", "%(refname:lstrip=2)"])
            .context("getting branch names")
    }

    pub fn branches(&self) -> Result<Vec<Branch<'_>>> {
        let mut res = Vec::new();
        for name in self.branch_names()?.drain(..) {
            res.push(Branch::new(name, self));
        }

        Ok(res)
    }

    pub(crate) fn branch_default(&self) -> Result<Branch<'_>> {
        Ok(Branch::new("main", self))
    }

    pub(crate) fn branch_create(&self, name: &str) -> Result<Branch<'_>> {
        self.cmd_check(["switch", "--create", name])?
            .true_or(anyhow!("creating branch failed"))?;
        Ok(Branch::new(name, self))
    }

    pub fn fork_point<T: AsRef<str>, S: AsRef<str>>(
        &self,
        name: T,
        base: S,
    ) -> Result<Option<String>> {
        let name: &str = name.as_ref();
        let other: &str = base.as_ref();
        let res = self.cmd_output(["merge-base", "--fork-point", other, name])?;
        let res = res.trim();

        let fork_point = if res.is_empty() {
            None
        } else {
            Some(res.into())
        };

        Ok(fork_point)
    }

    pub fn branch_head<T: AsRef<str>>(&self, name: T) -> Result<String> {
        let name: &str = name.as_ref();
        let res = self.cmd_output(["rev-parse", name])?;
        let res = res.trim();

        Ok(res.into())
    }

    #[expect(unused)]
    fn checkout(&self, commit: &str) -> Result<()> {
        self.git()
            .arg("checkout")
            .arg(commit)
            .status()?
            .success()
            .true_or(anyhow!("error checking out commit"))
    }

    pub fn merge_base<T: AsRef<str>, S: AsRef<str>>(&self, branch: T, other: S) -> Result<String> {
        let branch: &str = branch.as_ref();
        let other: &str = other.as_ref();
        let res = self.cmd_output(["merge-base", other, branch])?;
        let res = res.trim();

        Ok(res.into())
    }
}

impl<'a> Branch<'a> {
    pub fn new<T: AsRef<str>>(name: T, repo: &'a Repo) -> Self {
        let name = name.as_ref().to_string();
        let mut res = Self {
            name,
            repo,
            state: Default::default(),
        };

        res.load_state().ok();
        res
    }

    pub fn name(&'a self) -> &'a String {
        &self.name
    }

    pub fn fork_point<T: AsRef<str>>(&self, other: T) -> Result<Option<String>> {
        self.repo.fork_point(self.name(), other.as_ref())
    }

    #[expect(unused)]
    pub fn merge_base<T: AsRef<str>>(&self, other: T) -> Result<String> {
        self.repo.merge_base(self.name(), other)
    }

    fn state_file(&self) -> Utf8PathBuf {
        let slug = self.name.replace("/", "__");
        self.repo.git_dir().join("regit").join(slug)
    }

    pub fn load_state(&mut self) -> Result<()> {
        let state_file = self.state_file();
        let state: BranchState = read_from_file(state_file)?;
        self.state = state;
        Ok(())
    }

    pub fn save_state(&mut self) -> Result<()> {
        let state_file = self.state_file();
        write_to_file(state_file, &self.state)?;
        Ok(())
    }

    pub fn deps(&self) -> Vec<String> {
        self.state.deps.iter().cloned().collect_vec()
    }

    pub fn needs_update(&self) -> Result<bool> {
        for dep in self.state.deps.iter() {
            let fork_point = self.fork_point(dep)?;
            if let Some(fork_point) = fork_point {
                println!("fork point of {} on {} is {}", self.name(), dep, fork_point);
                let dep_head = self.repo.branch_head(dep)?;
                println!("head of {dep} is {}", self.repo.branch_head(dep)?);

                if dep_head != fork_point {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn update(&self) -> Result<()> {
        let deps = self.deps();
        if deps.is_empty() {
            println!(
                "branch {} does not have deps, no update needed.",
                self.name()
            );
            return Ok(());
        }

        if deps.len() > 1 {
            return Err(anyhow!(
                "branch `{}` has more than one dependency, which is currently unsupported.",
                self.name()
            ));
        }

        let dep = deps.first().unwrap();

        let fork_point = self.fork_point(dep)?.ok_or(anyhow!(
            "cannot determine fork point of {} with {dep}",
            self.name()
        ))?;

        let dep_head = self.repo.branch_head(dep)?;
        if dep_head == fork_point {
            println!("branch {}: no update needed.", self.name());
            return Ok(());
        }

        self.rebase_on(dep)?;

        Ok(())
    }

    fn rebase_on(&self, dep: &str) -> Result<()> {
        self.repo.cmd_check(["rebase", dep, self.name()])?;
        Ok(())
    }
}

trait TrueOr {
    fn true_or(self, error: anyhow::Error) -> Result<()>;
}

impl TrueOr for bool {
    fn true_or(self, error: anyhow::Error) -> Result<()> {
        if self {
            Ok(())
        } else {
            Err(error)
        }
    }
}

fn read_from_file<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `T`.
    let u = serde_json::from_reader(reader)?;

    // Return the `T`.
    Ok(u)
}

fn write_to_file<P: AsRef<Path>, T: Serialize>(path: P, val: &T) -> Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, val)?;

    Ok(())
}
