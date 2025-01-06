use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use petgraph::{
    acyclic::Acyclic,
    graph::{DiGraph, NodeIndex},
};

use crate::git::Repo;

pub type BranchGraph = DiGraph<String, ()>;

#[derive(Debug)]
pub struct GraphRepo {
    branch_map: IndexMap<String, NodeIndex>,
    pub graph: Acyclic<BranchGraph>,
}

impl GraphRepo {
    pub fn new(repo: &Repo) -> Result<Self> {
        let branches = repo.branches()?;

        let mut graph = BranchGraph::new();

        let mut branch_map = IndexMap::new();

        for branch in &branches {
            let index = graph.add_node(branch.name().clone());
            branch_map.insert(branch.name().clone(), index);
        }

        for branch in &branches {
            let branch_index = branch_map.get(branch.name()).unwrap();
            for dep in branch.deps() {
                if let Some(dep_index) = branch_map.get(&dep) {
                    graph.add_edge(*branch_index, *dep_index, ());
                } else {
                    println!(
                        "warning: branch `{}` depends on non-existing branch `{dep}`",
                        branch.name()
                    );
                }
            }
        }

        let acyclic = Acyclic::try_from_graph(graph).unwrap();
        Ok(Self {
            branch_map,
            graph: acyclic,
        })
    }

    fn branch_id<T: AsRef<str>>(&self, branch: T) -> Result<&NodeIndex> {
        let branch = branch.as_ref();
        self.branch_map
            .get(branch)
            .ok_or(anyhow!("branch `{branch}` not found"))
    }

    pub fn try_add_dep<T: AsRef<str>, S: AsRef<str>>(&mut self, branch: T, dep: S) -> Result<()> {
        let branch = branch.as_ref();
        let dep = dep.as_ref();

        self.graph
            .try_add_edge(*self.branch_id(branch)?, *self.branch_id(dep)?, ())
            .map_err(|_| {
                anyhow!("adding `{dep}` as dependency of `{branch}` would create a cycle")
            })?;

        Ok(())
    }
}
