use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use petgraph::{
    acyclic::Acyclic,
    graph::{DiGraph, NodeIndex},
    Direction::{self, Incoming, Outgoing},
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

    pub fn branch_id<T: AsRef<str>>(&self, branch: T) -> Result<&NodeIndex> {
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

    fn get_neighbors<T: AsRef<str>>(&self, branch: T, dir: Direction) -> Result<Vec<String>> {
        let branch = branch.as_ref();
        let mut neighbors = Vec::new();
        for id in self.graph.neighbors_directed(*self.branch_id(branch)?, dir) {
            neighbors.push(self.graph[id].clone());
        }

        Ok(neighbors)
    }

    pub fn get_dependencies<T: AsRef<str>>(&self, branch: T) -> Result<Vec<String>> {
        self.get_neighbors(branch, Outgoing)
    }

    pub fn get_dependents<T: AsRef<str>>(&self, branch: T) -> Result<Vec<String>> {
        self.get_neighbors(branch, Incoming)
    }

    pub fn reversed(&self) -> Self {
        let branch_map = self.branch_map.clone();

        let mut graph = self.graph.clone().into_inner();
        graph.reverse();

        let graph = Acyclic::try_from_graph(graph).unwrap();

        Self { branch_map, graph }
    }
}
