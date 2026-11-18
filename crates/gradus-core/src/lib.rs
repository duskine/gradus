//! Task graph and execution engine for Gradus.

use std::collections::{HashMap, VecDeque};

use gradus_model::TaskId;

/// A node in the task graph.
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: TaskId,
    pub depends_on: Vec<TaskId>,
}

/// A task graph: nodes plus validated edges.
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

    /// Return a topological ordering of the graph, or the set of nodes
    /// involved in a cycle if one exists.
    pub fn topological_order(&self) -> Result<Vec<TaskId>, Vec<TaskId>> {
        let mut indegree: HashMap<&TaskId, usize> =
            self.nodes.iter().map(|n| (&n.id, n.depends_on.len())).collect();

        let mut dependents: HashMap<&TaskId, Vec<&TaskId>> = HashMap::new();
        for node in &self.nodes {
            for dep in &node.depends_on {
                dependents.entry(dep).or_default().push(&node.id);
            }
        }

        let mut ready: VecDeque<&TaskId> =
            self.nodes.iter().filter(|n| indegree[&n.id] == 0).map(|n| &n.id).collect();

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
            let cycle = self.nodes.iter().filter(|n| !order.contains(&n.id)).map(|n| n.id.clone()).collect();
            Err(cycle)
        }
    }
}
