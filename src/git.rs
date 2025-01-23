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
    pub base: Option<String>,
    pub dirty: bool,
}

impl Repo {
    pub fn new() -> Repo {
        let git_dir = Repo::get_git_dir().unwrap();
        std::fs::create_dir_all(git_dir.join("giddy")).unwrap();
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
        Ok(Branch::new(self.default_branch_name(), self))
    }

    pub(crate) fn default_branch_name(&self) -> String {
        // TODO: get actual default branch name
        String::from("main")
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

    pub fn get_base_branch<T: AsRef<str>>(&self, branch: T) -> Result<String> {
        let default_branch = self.default_branch_name();
        let branch = branch.as_ref();
        let branch_head = self.branch_head(branch)?;

        if branch_head == self.branch_head(&default_branch)? {
            return Ok(default_branch);
        }

        let fork_point = self.fork_point(branch, &default_branch)?.ok_or_else(||anyhow!("cannot determine fork point between `{branch}` and the default branch `{default_branch}`. has it been merged?"))?;

        let mut log = self.cmd_output_vec([
            "log",
            "--format=%H %D",
            "--decorate=full",
            &format!("{fork_point}..{branch_head}"),
        ])?;

        for (_hash, branches) in log.iter_mut().filter_map(|line| line.split_once(' ')) {
            let branch = branches
                .split(", ")
                .filter(|b| !b.starts_with("HEAD ->"))
                .filter_map(|branch| branch.strip_prefix("refs/heads/"))
                .filter(|b| b != &branch)
                .take(1)
                .next();

            if let Some(branch) = branch {
                return Ok(branch.to_string());
            }
        }

        Ok(default_branch)
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

    pub fn contains<T: AsRef<str>, S: AsRef<str>>(&self, branch: T, contains: S) -> Result<bool> {
        let branch: &str = branch.as_ref();
        let contains: &str = contains.as_ref();

        let results = self
            .cmd_output_vec([
                "branch",
                "--format=%(refname)",
                "--contains",
                contains,
                branch,
            ])
            .with_context(|| format!("checking if `{branch}` contains `{contains}`"))?;

        Ok(results
            .first()
            .is_some_and(|first| first == &format!("refs/heads/{branch}")))
    }

    pub fn merged<T: AsRef<str>, S: AsRef<str>>(&self, branch: T, has_merged: S) -> Result<bool> {
        let branch: &str = branch.as_ref();
        let has_merged: &str = has_merged.as_ref();

        let results = self
            .cmd_output_vec([
                "branch",
                "--format=%(refname)",
                "--merged",
                branch,
                has_merged,
            ])
            .with_context(|| format!("checking if `{branch}` has merged `{has_merged}`"))?;

        Ok(results
            .first()
            .is_some_and(|first| first == &format!("refs/heads/{has_merged}")))
    }

    pub fn equal<T: AsRef<str>, S: AsRef<str>>(&self, branch: T, other: S) -> Result<bool> {
        let branch: &str = branch.as_ref();
        let other: &str = other.as_ref();

        Ok(self.branch_head(branch)? == self.branch_head(other)?)
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
        if res.state.base.is_none() && res.name != repo.default_branch_name() {
            res.state.base = Some(repo.default_branch_name());
        }
        res
    }

    pub fn name(&'a self) -> &'a String {
        &self.name
    }

    pub fn head(&self) -> Result<String> {
        self.repo.branch_head(&self.name)
    }

    pub fn equal<T: AsRef<str>>(&self, other: T) -> Result<bool> {
        self.repo.equal(&self.name, other)
    }

    pub fn merged_into<T: AsRef<str>>(&self, other: T) -> Result<bool> {
        let other = other.as_ref();
        Ok(self.repo.merged(other, &self.name)?)
    }

    pub fn merged(&self) -> Result<bool> {
        let base = self.state.base.as_ref().ok_or(anyhow!("no base branch"))?;
        self.merged_into(&base)
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
        self.repo.git_dir().join("giddy").join(slug)
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
        if self.state.deps.is_empty() {
            let default_branch_name = self.repo.default_branch_name();
            if self.name == default_branch_name {
                Vec::new()
            } else {
                vec![self.repo.default_branch_name()]
            }
        } else {
            self.state.deps.iter().cloned().collect_vec()
        }
    }

    pub fn only_default_deps(&self) -> bool {
        self.state.deps.is_empty()
            || (self.state.deps.len() == 1
                && self.state.deps.first().unwrap() == &self.repo.default_branch_name())
    }

    pub fn needs_update(&self) -> Result<bool> {
        for dep in self.state.deps.iter() {
            let fork_point = self.fork_point(dep)?;
            if let Some(fork_point) = fork_point {
                //println!("fork point of {} on {} is {}", self.name(), dep, fork_point);
                let dep_head = self.repo.branch_head(dep)?;
                //println!("head of {dep} is {}", self.repo.branch_head(dep)?);

                if dep_head != fork_point {
                    return Ok(true);
                }
            } else {
                // no fork point found, probably the base branch or deps changed
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn update(&mut self) -> Result<()> {
        let mut deps = self.deps();
        if deps.is_empty() {
            println!(
                "branch {} does not have deps, no update needed.",
                self.name()
            );
            return Ok(());
        }

        if deps.len() > 1 {
            deps.retain(|dep| dep != &self.repo.default_branch_name());
            self.state
                .deps
                .shift_remove(&self.repo.default_branch_name());
            self.save_state()?;
        }

        if deps.len() > 1 {
            return Err(anyhow!(
                "branch `{}` has more than one dependency, which is currently unsupported.",
                self.name()
            ));
        }

        let dep = deps.first().unwrap();
        if let Some(previous) = self.state.base.as_ref().cloned() {
            if dep != &previous {
                println!(
                    "branch `{}`: rebasing from `{}` onto `{}`...",
                    self.name, previous, dep
                );

                // TODO: check if new base is dirty

                self.rebase_onto(&previous, dep)?;
                self.state.base = Some(dep.clone());
                self.state.dirty = false;
                self.save_state()?;
                return Ok(());
            }
        }

        let skip_update;

        let dep_head = self.repo.branch_head(dep)?;
        let fork_point = self.fork_point(dep)?;

        if let Some(fork_point) = &fork_point {
            skip_update = &dep_head == fork_point;
        } else {
            // TODO: reflog

            let branch_head = self.head()?;

            skip_update = (branch_head == dep_head)
                || self.repo.contains(dep, &self.name)?
                || self.repo.merged(dep, &self.name)?;
        }

        if skip_update {
            println!("branch {}: no update needed.", self.name());
        } else if let Some(fork_point) = &fork_point {
            println!("rebasing branch `{}` on `{dep}`...", self.name());
            self.rebase_onto(fork_point, dep)?;
        } else {
            return Err(anyhow!(
                "unable to determine fork point between `{}` and `{}`!",
                self.name(),
                dep
            ));
        }

        Ok(())
    }

    fn rebase_on(&self, dep: &str) -> Result<()> {
        self.repo.cmd_check(["rebase", dep, self.name()])?;
        Ok(())
    }

    fn rebase_onto(&mut self, old: &str, new: &str) -> Result<()> {
        self.repo
            .cmd_check(["rebase", "--onto", new, old, self.name()])?;
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
