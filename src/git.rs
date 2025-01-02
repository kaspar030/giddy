use std::{
    ffi::OsStr,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
    process::Command,
};

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug)]
pub struct Repo {
    git_dir: Utf8PathBuf,
}

impl Repo {
    pub fn new() -> Repo {
        let git_dir = Repo::get_git_dir().unwrap();
        std::fs::create_dir_all(git_dir.join("regit")).unwrap();
        Repo { git_dir }
    }

    pub fn git(&self) -> std::process::Command {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.git_dir);

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
        let res = String::from_utf8(output.stdout).expect("failed to convert remote url into utf8");

        Ok(res)
    }

    fn checkout(&self, commit: &str) -> Result<()> {
        self.git()
            .arg("checkout")
            .arg(commit)
            .status()?
            .success()
            .true_or(anyhow!("error checking out commit"))
    }

    pub fn branch_current(&self) -> Result<Branch<'_>> {
        let name = self.cmd_output(["branch", "--show-current"])?;
        let name = name.trim();

        Ok(Branch::new(name, self))
    }
}

#[derive(Debug)]
pub struct Branch<'a> {
    name: String,
    repo: &'a Repo,
    pub state: BranchState,
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

    pub fn merge_base<T: AsRef<str>>(&self, other: T) -> Result<String> {
        let other: &str = other.as_ref();
        let res = self.repo.cmd_output(["merge-base", other, &self.name])?;
        let res = res.trim();

        Ok(res.into())
    }

    fn state_file(&self) -> Utf8PathBuf {
        self.repo.git_dir().join("regit").join(&self.name)
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
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BranchState {
    pub deps: Vec<String>,
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

    serde_json::to_writer(writer, val)?;

    Ok(())
}
