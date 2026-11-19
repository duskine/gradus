//! Task graph and execution engine for Gradus.

use std::collections::{HashMap, HashSet, VecDeque};

use gradus_model::{BuildFile, TaskId};

#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
}

#[derive(Debug, Default, Clone)]
pub struct TaskGraph {
    nodes: Vec<TaskNode>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, node: TaskNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[TaskNode] {
        &self.nodes
    }

    pub fn topological_order(&self) -> Result<Vec<TaskId>, Vec<TaskId>> {
        let mut indegree: HashMap<&TaskId, usize> = self
            .nodes
            .iter()
            .map(|n| (&n.id, n.depends_on.len()))
            .collect();

        let mut dependents: HashMap<&TaskId, Vec<&TaskId>> = HashMap::new();
        for node in &self.nodes {
            for dep in &node.depends_on {
                dependents.entry(dep).or_default().push(&node.id);
            }
        }

        let mut ready: VecDeque<&TaskId> = self
            .nodes
            .iter()
            .filter(|n| indegree[&n.id] == 0)
            .map(|n| &n.id)
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_front() {
            order.push(id.clone());
            if let Some(deps) = dependents.get(id) {
                for d in deps {
                    if let Some(e) = indegree.get_mut(*d) {
                        *e -= 1;
                        if *e == 0 {
                            ready.push_back(*d);
                        }
                    }
                }
            }
        }

        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            let cycle: Vec<TaskId> = self
                .nodes
                .iter()
                .filter(|n| !order.contains(&n.id))
                .map(|n| n.id.clone())
                .collect();
            Err(cycle)
        }
    }

    /// Return the tasks that must run to satisfy `root`, in topological order.
    pub fn resolve(&self, root: &TaskId) -> Result<Vec<TaskId>, Vec<TaskId>> {
        let mut needed = HashSet::new();
        self.collect_deps(root, &mut needed);
        let order = self.topological_order()?;
        Ok(order.into_iter().filter(|id| needed.contains(id)).collect())
    }

    fn collect_deps(&self, id: &TaskId, acc: &mut HashSet<TaskId>) {
        if !acc.insert(id.clone()) {
            return;
        }
        if let Some(node) = self.nodes.iter().find(|n| &n.id == id) {
            for dep in &node.depends_on {
                self.collect_deps(dep, acc);
            }
        }
    }
}

/// Derive the built-in task graph from a parsed build file.
///
/// The graph is fixed in 0.1.2: `clean`, `compile`, `jar`, `test`, `build`.
/// Future config sections (`[tasks.*]`) will modify it.
pub fn graph_from_build_file(_build: &BuildFile) -> TaskGraph {
    let clean = TaskId::new("", "clean");
    let compile = TaskId::new("", "compile");
    let test = TaskId::new("", "test");
    let jar = TaskId::new("", "jar");
    let build = TaskId::new("", "build");

    let mut graph = TaskGraph::new();
    graph.add(TaskNode {
        id: clean.clone(),
        depends_on: vec![],
    });
    graph.add(TaskNode {
        id: compile.clone(),
        depends_on: vec![],
    });
    graph.add(TaskNode {
        id: test.clone(),
        depends_on: vec![compile.clone()],
    });
    graph.add(TaskNode {
        id: jar,
        depends_on: vec![compile],
    });
    graph.add(TaskNode {
        id: build,
        depends_on: vec![jar, test],
    });
    graph
}