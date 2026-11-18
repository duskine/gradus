use gradus_core::{TaskGraph, TaskNode};
use gradus_model::TaskId;

#[test]
fn empty_graph_is_valid() {
    let g = TaskGraph::new();
    assert!(g.topological_order().unwrap().is_empty());
}

#[test]
fn linear_chain_orders_correctly() {
    let mut g = TaskGraph::new();
    g.add(TaskNode { id: TaskId::new("", "clean"), depends_on: vec![] });
    g.add(TaskNode { id: TaskId::new("", "compile"), depends_on: vec![TaskId::new("", "clean")] });
    g.add(TaskNode { id: TaskId::new("", "test"), depends_on: vec![TaskId::new("", "compile")] });

    let order = g.topological_order().unwrap();
    assert_eq!(order.first(), Some(&TaskId::new("", "clean")));
    assert_eq!(order.last(), Some(&TaskId::new("", "test")));
}

#[test]
fn two_cycles_are_reported_not_hidden() {
    let mut g = TaskGraph::new();
    g.add(TaskNode { id: TaskId::new("", "a"), depends_on: vec![TaskId::new("", "b")] });
    g.add(TaskNode { id: TaskId::new("", "b"), depends_on: vec![TaskId::new("", "a")] });

    let cycle = g.topological_order().unwrap_err();
    assert_eq!(cycle.len(), 2);
}

#[test]
fn diamond_graph_orders_correctly() {
    let mut g = TaskGraph::new();
    g.add(TaskNode { id: TaskId::new("", "a"), depends_on: vec![] });
    g.add(TaskNode { id: TaskId::new("", "b"), depends_on: vec![TaskId::new("", "a")] });
    g.add(TaskNode { id: TaskId::new("", "c"), depends_on: vec![TaskId::new("", "a")] });
    g.add(TaskNode { id: TaskId::new("", "d"), depends_on: vec![TaskId::new("", "b"), TaskId::new("", "c")] });

    let order = g.topological_order().unwrap();
    let pos = |name: &str| order.iter().position(|t| t.task == name).unwrap();

    assert!(pos("a") < pos("b"));
    assert!(pos("a") < pos("c"));
    assert!(pos("b") < pos("d"));
    assert!(pos("c") < pos("d"));
}
