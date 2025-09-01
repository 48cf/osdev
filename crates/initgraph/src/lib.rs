#![no_std]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use intrusive_collections::{LinkedList, LinkedListAtomicLink, intrusive_adapter};
use spin::Mutex;

// Re-export the proc-macro.
pub use initgraph_proc::task;

intrusive_adapter! {
    InLinkAdapter = &'static Edge: Edge { in_link: LinkedListAtomicLink }
}

intrusive_adapter! {
    OutLinkAdapter = &'static Edge: Edge { out_link: LinkedListAtomicLink }
}

intrusive_adapter! {
    PendingLinkAdapter = &'static Node: Node { pending_link: LinkedListAtomicLink }
}

pub enum NodeAction {
    Nothing,
    Callback(fn()),
}

impl NodeAction {
    fn execute(&self) {
        match self {
            NodeAction::Nothing => {}
            NodeAction::Callback(cb) => cb(),
        }
    }
}

pub struct Edge {
    source: &'static Node,
    target: &'static Node,

    in_link: LinkedListAtomicLink,
    out_link: LinkedListAtomicLink,
}

impl Edge {
    pub const fn new(source: &'static Node, target: &'static Node) -> Self {
        Self {
            source,
            target,
            in_link: LinkedListAtomicLink::new(),
            out_link: LinkedListAtomicLink::new(),
        }
    }

    #[doc(hidden)]
    pub fn register(&'static self) {
        self.source.out_edges.lock().push_back(self);
        self.target.in_edges.lock().push_back(self);
        self.target.unsatisfied_deps.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct Node {
    name: &'static str,
    action: NodeAction,

    unsatisfied_deps: AtomicUsize,
    wanted: AtomicBool,
    done: AtomicBool,

    in_edges: Mutex<LinkedList<InLinkAdapter>>,
    out_edges: Mutex<LinkedList<OutLinkAdapter>>,
    pending_link: LinkedListAtomicLink,
}

impl Node {
    pub const fn new(name: &'static str, action: NodeAction) -> Self {
        Self {
            name,
            action,
            unsatisfied_deps: AtomicUsize::new(0),
            wanted: AtomicBool::new(false),
            done: AtomicBool::new(false),
            in_edges: Mutex::new(LinkedList::new(InLinkAdapter::NEW)),
            out_edges: Mutex::new(LinkedList::new(OutLinkAdapter::NEW)),
            pending_link: LinkedListAtomicLink::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}

unsafe extern "C" {
    static LD_INIT_NODES_START: u8;
    static LD_INIT_NODES_END: u8;
    static LD_INIT_CTORS_START: u8;
    static LD_INIT_CTORS_END: u8;
}

/// # Safety
/// This function must be called exactly once during initialization.
pub unsafe fn register_edges() {
    let ctors = unsafe {
        let ctors_start = &raw const LD_INIT_CTORS_START as *const fn();
        let ctors_end = &raw const LD_INIT_CTORS_END as *const fn();

        core::slice::from_raw_parts(ctors_start, ctors_end.offset_from_unsigned(ctors_start))
    };

    for ctor in ctors {
        ctor();
    }
}

pub fn execute_graph<F: Fn(&'static Node)>(goal: Option<&'static Node>, on_node_reached: F) {
    let nodes = unsafe {
        let nodes_start = &raw const LD_INIT_NODES_START as *const Node;
        let nodes_end = &raw const LD_INIT_NODES_END as *const Node;

        core::slice::from_raw_parts(nodes_start, nodes_end.offset_from_unsigned(nodes_start))
    };

    if let Some(goal) = goal {
        let mut queue = LinkedList::new(PendingLinkAdapter::NEW);

        if !goal.wanted.swap(true, Ordering::Relaxed) {
            queue.push_back(goal);
        }

        while let Some(node) = queue.pop_front() {
            for edge in node.in_edges.lock().iter() {
                if !edge.source.wanted.swap(true, Ordering::Relaxed) {
                    queue.push_back(edge.source);
                }
            }
        }
    } else {
        for node in nodes {
            node.wanted.store(true, Ordering::Relaxed);
        }
    }

    let mut pending = LinkedList::new(PendingLinkAdapter::NEW);

    for node in nodes.iter().filter(|node| {
        node.wanted.load(Ordering::Relaxed)
            && !node.done.load(Ordering::Relaxed)
            && node.unsatisfied_deps.load(Ordering::Relaxed) == 0
    }) {
        pending.push_back(node);
    }

    while let Some(node) = pending.pop_front() {
        on_node_reached(node);

        node.action.execute();
        node.done.store(true, Ordering::Relaxed);

        for edge in node.out_edges.lock().iter() {
            let prev = edge.target.unsatisfied_deps.fetch_sub(1, Ordering::Relaxed);

            assert!(prev > 0);

            if edge.target.wanted.load(Ordering::Relaxed)
                && !edge.target.done.load(Ordering::Relaxed)
                && prev == 1
            {
                pending.push_back(edge.target);
            }
        }
    }

    if let Some(node) = nodes
        .iter()
        .find(|node| node.wanted.load(Ordering::Relaxed) && !node.done.load(Ordering::Relaxed))
    {
        panic!(
            "initgraph: Node {} was not reached because its dependencies were not satisfied",
            node.name()
        );
    }
}
