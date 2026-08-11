//! std::collections — Set, Deque, PriorityQueue, OrderedMap, Counter, Graph.
//!
//! Estructuras de datos serias que faltaban en la stdlib para escribir
//! algoritmos profesionales. Cada una implementada con la primitiva
//! correcta de Rust std / indexmap y con las operaciones completas que
//! espera un dev: no son wrappers minimales, son abstracciones útiles.

use std::collections::{BTreeSet, VecDeque, BinaryHeap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::IndexMap;

// ---------------- Global handle registry ----------------
//
// Todas las estructuras se guardan en un registro global con handle
// int64 — igual patron que sqlite/websocket handles en Titan.

static NEXT_HANDLE: OnceLock<AtomicU64> = OnceLock::new();
fn next_handle() -> u64 {
    NEXT_HANDLE.get_or_init(|| AtomicU64::new(1)).fetch_add(1, Ordering::Relaxed)
}

fn handle_key(handle: u64) -> (u64, u64) { crate::native::runtime_handle_key(handle) }

// ---------------- Set (BTreeSet<String>) ----------------

static SETS: OnceLock<Mutex<HashMap<(u64, u64), BTreeSet<String>>>> = OnceLock::new();
fn sets() -> &'static Mutex<HashMap<(u64, u64), BTreeSet<String>>> {
    SETS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_new() -> u64 {
    let h = next_handle();
    sets().lock().unwrap().insert(handle_key(h), BTreeSet::new());
    h
}

pub fn set_from(items: Vec<String>) -> u64 {
    let h = next_handle();
    sets().lock().unwrap().insert(handle_key(h), items.into_iter().collect());
    h
}

pub fn set_add(h: u64, item: String) -> Result<bool, String> {
    let mut sets = sets().lock().unwrap();
    let s = sets.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown set {h}"))?;
    Ok(s.insert(item))
}

pub fn set_remove(h: u64, item: &str) -> Result<bool, String> {
    let mut sets = sets().lock().unwrap();
    let s = sets.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown set {h}"))?;
    Ok(s.remove(item))
}

pub fn set_contains(h: u64, item: &str) -> Result<bool, String> {
    let sets = sets().lock().unwrap();
    let s = sets.get(&handle_key(h)).ok_or_else(|| format!("unknown set {h}"))?;
    Ok(s.contains(item))
}

pub fn set_len(h: u64) -> Result<usize, String> {
    let sets = sets().lock().unwrap();
    let s = sets.get(&handle_key(h)).ok_or_else(|| format!("unknown set {h}"))?;
    Ok(s.len())
}

pub fn set_to_array(h: u64) -> Result<Vec<String>, String> {
    let sets = sets().lock().unwrap();
    let s = sets.get(&handle_key(h)).ok_or_else(|| format!("unknown set {h}"))?;
    Ok(s.iter().cloned().collect())
}

pub fn set_union(a: u64, b: u64) -> Result<u64, String> {
    let sets_g = sets().lock().unwrap();
    let sa = sets_g.get(&handle_key(a)).ok_or_else(|| format!("unknown set {a}"))?;
    let sb = sets_g.get(&handle_key(b)).ok_or_else(|| format!("unknown set {b}"))?;
    let merged: BTreeSet<String> = sa.union(sb).cloned().collect();
    drop(sets_g);
    let h = next_handle();
    sets().lock().unwrap().insert(handle_key(h), merged);
    Ok(h)
}

pub fn set_intersect(a: u64, b: u64) -> Result<u64, String> {
    let sets_g = sets().lock().unwrap();
    let sa = sets_g.get(&handle_key(a)).ok_or_else(|| format!("unknown set {a}"))?;
    let sb = sets_g.get(&handle_key(b)).ok_or_else(|| format!("unknown set {b}"))?;
    let merged: BTreeSet<String> = sa.intersection(sb).cloned().collect();
    drop(sets_g);
    let h = next_handle();
    sets().lock().unwrap().insert(handle_key(h), merged);
    Ok(h)
}

pub fn set_difference(a: u64, b: u64) -> Result<u64, String> {
    let sets_g = sets().lock().unwrap();
    let sa = sets_g.get(&handle_key(a)).ok_or_else(|| format!("unknown set {a}"))?;
    let sb = sets_g.get(&handle_key(b)).ok_or_else(|| format!("unknown set {b}"))?;
    let merged: BTreeSet<String> = sa.difference(sb).cloned().collect();
    drop(sets_g);
    let h = next_handle();
    sets().lock().unwrap().insert(handle_key(h), merged);
    Ok(h)
}

pub fn set_is_subset(a: u64, b: u64) -> Result<bool, String> {
    let sets_g = sets().lock().unwrap();
    let sa = sets_g.get(&handle_key(a)).ok_or_else(|| format!("unknown set {a}"))?;
    let sb = sets_g.get(&handle_key(b)).ok_or_else(|| format!("unknown set {b}"))?;
    Ok(sa.is_subset(sb))
}

pub fn set_drop(h: u64) -> bool {
    sets().lock().unwrap().remove(&handle_key(h)).is_some()
}

// ---------------- Deque (VecDeque<String>) ----------------

static DEQUES: OnceLock<Mutex<HashMap<(u64, u64), VecDeque<String>>>> = OnceLock::new();
fn deques() -> &'static Mutex<HashMap<(u64, u64), VecDeque<String>>> {
    DEQUES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn deque_new() -> u64 {
    let h = next_handle();
    deques().lock().unwrap().insert(handle_key(h), VecDeque::new());
    h
}

pub fn deque_push_front(h: u64, item: String) -> Result<(), String> {
    let mut d = deques().lock().unwrap();
    let q = d.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    q.push_front(item);
    Ok(())
}

pub fn deque_push_back(h: u64, item: String) -> Result<(), String> {
    let mut d = deques().lock().unwrap();
    let q = d.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    q.push_back(item);
    Ok(())
}

pub fn deque_pop_front(h: u64) -> Result<Option<String>, String> {
    let mut d = deques().lock().unwrap();
    let q = d.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(q.pop_front())
}

pub fn deque_pop_back(h: u64) -> Result<Option<String>, String> {
    let mut d = deques().lock().unwrap();
    let q = d.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(q.pop_back())
}

pub fn deque_len(h: u64) -> Result<usize, String> {
    let d = deques().lock().unwrap();
    let q = d.get(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(q.len())
}

pub fn deque_to_array(h: u64) -> Result<Vec<String>, String> {
    let d = deques().lock().unwrap();
    let q = d.get(&handle_key(h)).ok_or_else(|| format!("unknown deque {h}"))?;
    Ok(q.iter().cloned().collect())
}

pub fn deque_drop(h: u64) -> bool {
    deques().lock().unwrap().remove(&handle_key(h)).is_some()
}

// ---------------- PriorityQueue (BinaryHeap) ----------------
//
// Wrapper con flag min/max. Internamente usa BinaryHeap<(prioridad, seq, item)>.
// El seq garantiza FIFO cuando hay empate — comportamiento estable esperado.

struct PQ {
    is_min: bool,
    heap: BinaryHeap<(i64, i64, String)>,  // prioridad_ajustada, -seq, item
    next_seq: i64,
}

static PQS: OnceLock<Mutex<HashMap<(u64, u64), PQ>>> = OnceLock::new();
fn pqs() -> &'static Mutex<HashMap<(u64, u64), PQ>> { PQS.get_or_init(|| Mutex::new(HashMap::new())) }

pub fn pq_new_max() -> u64 {
    let h = next_handle();
    pqs().lock().unwrap().insert(handle_key(h), PQ { is_min: false, heap: BinaryHeap::new(), next_seq: 0 });
    h
}

pub fn pq_new_min() -> u64 {
    let h = next_handle();
    pqs().lock().unwrap().insert(handle_key(h), PQ { is_min: true, heap: BinaryHeap::new(), next_seq: 0 });
    h
}

pub fn pq_push(h: u64, item: String, priority: i64) -> Result<(), String> {
    let mut pqs = pqs().lock().unwrap();
    let pq = pqs.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown pq {h}"))?;
    let adj = if pq.is_min { -priority } else { priority };
    let seq = pq.next_seq;
    pq.next_seq += 1;
    pq.heap.push((adj, -seq, item));
    Ok(())
}

pub fn pq_pop(h: u64) -> Result<Option<String>, String> {
    let mut pqs = pqs().lock().unwrap();
    let pq = pqs.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown pq {h}"))?;
    Ok(pq.heap.pop().map(|(_, _, item)| item))
}

pub fn pq_peek(h: u64) -> Result<Option<String>, String> {
    let pqs = pqs().lock().unwrap();
    let pq = pqs.get(&handle_key(h)).ok_or_else(|| format!("unknown pq {h}"))?;
    Ok(pq.heap.peek().map(|(_, _, item)| item.clone()))
}

pub fn pq_len(h: u64) -> Result<usize, String> {
    let pqs = pqs().lock().unwrap();
    let pq = pqs.get(&handle_key(h)).ok_or_else(|| format!("unknown pq {h}"))?;
    Ok(pq.heap.len())
}

pub fn pq_drop(h: u64) -> bool { pqs().lock().unwrap().remove(&handle_key(h)).is_some() }

// ---------------- OrderedMap (IndexMap<String, serde_json::Value>) ----------------
//
// Map que preserva orden de inserción. Los valores son serde_json::Value
// para poder guardar cualquier cosa; el Value se convierte a Titan Value
// en el layer del VM.

static OMAPS: OnceLock<Mutex<HashMap<(u64, u64), IndexMap<String, serde_json::Value>>>> = OnceLock::new();
fn omaps() -> &'static Mutex<HashMap<(u64, u64), IndexMap<String, serde_json::Value>>> {
    OMAPS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn omap_new() -> u64 {
    let h = next_handle();
    omaps().lock().unwrap().insert(handle_key(h), IndexMap::new());
    h
}

pub fn omap_insert(h: u64, key: String, value: serde_json::Value) -> Result<(), String> {
    let mut o = omaps().lock().unwrap();
    let m = o.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown omap {h}"))?;
    m.insert(key, value);
    Ok(())
}

pub fn omap_get(h: u64, key: &str) -> Result<Option<serde_json::Value>, String> {
    let o = omaps().lock().unwrap();
    let m = o.get(&handle_key(h)).ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(m.get(key).cloned())
}

pub fn omap_remove(h: u64, key: &str) -> Result<bool, String> {
    let mut o = omaps().lock().unwrap();
    let m = o.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(m.shift_remove(key).is_some())
}

pub fn omap_keys(h: u64) -> Result<Vec<String>, String> {
    let o = omaps().lock().unwrap();
    let m = o.get(&handle_key(h)).ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(m.keys().cloned().collect())
}

pub fn omap_len(h: u64) -> Result<usize, String> {
    let o = omaps().lock().unwrap();
    let m = o.get(&handle_key(h)).ok_or_else(|| format!("unknown omap {h}"))?;
    Ok(m.len())
}

pub fn omap_drop(h: u64) -> bool { omaps().lock().unwrap().remove(&handle_key(h)).is_some() }

// ---------------- Counter (frecuencia de items) ----------------
//
// Encima de HashMap<String, i64>. Ops típicas: from_array, count,
// most_common(n), total.

static COUNTERS: OnceLock<Mutex<HashMap<(u64, u64), HashMap<String, i64>>>> = OnceLock::new();
fn counters() -> &'static Mutex<HashMap<(u64, u64), HashMap<String, i64>>> {
    COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn counter_from(items: Vec<String>) -> u64 {
    let h = next_handle();
    let mut m: HashMap<String, i64> = HashMap::new();
    for i in items { *m.entry(i).or_insert(0) += 1; }
    counters().lock().unwrap().insert(handle_key(h), m);
    h
}

pub fn counter_add(h: u64, item: String, delta: i64) -> Result<(), String> {
    let mut c = counters().lock().unwrap();
    let m = c.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown counter {h}"))?;
    *m.entry(item).or_insert(0) += delta;
    Ok(())
}

pub fn counter_count(h: u64, item: &str) -> Result<i64, String> {
    let c = counters().lock().unwrap();
    let m = c.get(&handle_key(h)).ok_or_else(|| format!("unknown counter {h}"))?;
    Ok(*m.get(item).unwrap_or(&0))
}

pub fn counter_most_common(h: u64, n: usize) -> Result<Vec<(String, i64)>, String> {
    let c = counters().lock().unwrap();
    let m = c.get(&handle_key(h)).ok_or_else(|| format!("unknown counter {h}"))?;
    let mut v: Vec<(String, i64)> = m.iter().map(|(k, v)| (k.clone(), *v)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(n);
    Ok(v)
}

pub fn counter_total(h: u64) -> Result<i64, String> {
    let c = counters().lock().unwrap();
    let m = c.get(&handle_key(h)).ok_or_else(|| format!("unknown counter {h}"))?;
    Ok(m.values().sum())
}

pub fn counter_drop(h: u64) -> bool { counters().lock().unwrap().remove(&handle_key(h)).is_some() }

// ---------------- Graph (directed/undirected + algoritmos) ----------------

struct Graph {
    directed: bool,
    edges: HashMap<String, Vec<(String, i64)>>,  // node -> [(vecino, peso)]
    nodes: BTreeSet<String>,
}

static GRAPHS: OnceLock<Mutex<HashMap<(u64, u64), Graph>>> = OnceLock::new();
fn graphs() -> &'static Mutex<HashMap<(u64, u64), Graph>> {
    GRAPHS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn graph_new(directed: bool) -> u64 {
    let h = next_handle();
    graphs().lock().unwrap().insert(handle_key(h), Graph { directed, edges: HashMap::new(), nodes: BTreeSet::new() });
    h
}

pub fn graph_add_node(h: u64, node: String) -> Result<(), String> {
    let mut g = graphs().lock().unwrap();
    let graph = g.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    graph.nodes.insert(node.clone());
    graph.edges.entry(node).or_default();
    Ok(())
}

pub fn graph_add_edge(h: u64, from: String, to: String, weight: i64) -> Result<(), String> {
    let mut g = graphs().lock().unwrap();
    let graph = g.get_mut(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    graph.nodes.insert(from.clone());
    graph.nodes.insert(to.clone());
    graph.edges.entry(from.clone()).or_default().push((to.clone(), weight));
    if !graph.directed {
        graph.edges.entry(to).or_default().push((from, weight));
    } else {
        graph.edges.entry(to).or_default();
    }
    Ok(())
}

pub fn graph_neighbors(h: u64, node: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    Ok(graph.edges.get(node).map(|v| v.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default())
}

/// BFS: retorna nodos en orden de visita.
pub fn graph_bfs(h: u64, start: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) { return Ok(Vec::new()); }
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut order: Vec<String> = Vec::new();
    queue.push_back(start.to_string());
    visited.insert(start.to_string());
    while let Some(n) = queue.pop_front() {
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (nbr, _) in sorted {
                if !visited.contains(nbr) {
                    visited.insert(nbr.clone());
                    queue.push_back(nbr.clone());
                }
            }
        }
    }
    Ok(order)
}

/// DFS iterativo con stack.
pub fn graph_dfs(h: u64, start: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) { return Ok(Vec::new()); }
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![start.to_string()];
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = stack.pop() {
        if visited.contains(&n) { continue; }
        visited.insert(n.clone());
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            sorted.sort_by(|a, b| b.0.cmp(&a.0));  // reverse para que DFS visite en orden alfabético
            for (nbr, _) in sorted {
                if !visited.contains(nbr) {
                    stack.push(nbr.clone());
                }
            }
        }
    }
    Ok(order)
}

/// Dijkstra: shortest path desde start hasta end. Retorna la lista de
/// nodos en el camino (incluye start y end). Vacío si no hay camino.
pub fn graph_shortest_path(h: u64, start: &str, end: &str) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.nodes.contains(start) || !graph.nodes.contains(end) { return Ok(Vec::new()); }

    let mut dist: HashMap<String, i64> = HashMap::new();
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut heap: BinaryHeap<(std::cmp::Reverse<i64>, String)> = BinaryHeap::new();
    dist.insert(start.to_string(), 0);
    heap.push((std::cmp::Reverse(0), start.to_string()));

    while let Some((std::cmp::Reverse(d), u)) = heap.pop() {
        if u == end { break; }
        if d > *dist.get(&u).unwrap_or(&i64::MAX) { continue; }
        if let Some(nbrs) = graph.edges.get(&u) {
            for (v, w) in nbrs {
                let alt = d.saturating_add(*w);
                if alt < *dist.get(v).unwrap_or(&i64::MAX) {
                    dist.insert(v.clone(), alt);
                    prev.insert(v.clone(), u.clone());
                    heap.push((std::cmp::Reverse(alt), v.clone()));
                }
            }
        }
    }

    if !dist.contains_key(end) { return Ok(Vec::new()); }
    let mut path: Vec<String> = Vec::new();
    let mut cur = end.to_string();
    loop {
        path.push(cur.clone());
        if cur == start { break; }
        match prev.get(&cur) {
            Some(p) => cur = p.clone(),
            None    => return Ok(Vec::new()),
        }
    }
    path.reverse();
    Ok(path)
}

/// Topological sort. Retorna orden válido o Err si hay ciclo.
pub fn graph_topological_sort(h: u64) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    if !graph.directed { return Err("topological_sort requires directed graph".into()); }
    let mut in_degree: HashMap<String, i64> = HashMap::new();
    for n in &graph.nodes { in_degree.insert(n.clone(), 0); }
    for (_, nbrs) in &graph.edges {
        for (v, _) in nbrs {
            *in_degree.entry(v.clone()).or_insert(0) += 1;
        }
    }
    let mut queue: VecDeque<String> = in_degree.iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    let mut sorted_queue: Vec<String> = queue.iter().cloned().collect();
    sorted_queue.sort();
    queue = sorted_queue.into_iter().collect();
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = queue.pop_front() {
        order.push(n.clone());
        if let Some(nbrs) = graph.edges.get(&n) {
            let mut nbrs_sorted: Vec<&(String, i64)> = nbrs.iter().collect();
            nbrs_sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (v, _) in nbrs_sorted {
                let d = in_degree.entry(v.clone()).or_insert(0);
                *d -= 1;
                if *d == 0 { queue.push_back(v.clone()); }
            }
        }
    }
    if order.len() != graph.nodes.len() {
        return Err("cycle detected: topological sort not possible".into());
    }
    Ok(order)
}

/// Detecta si el grafo tiene ciclo (funciona en directed y undirected).
pub fn graph_has_cycle(h: u64) -> Result<bool, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    if graph.directed {
        // Uso DFS con 3 estados: unvisited, visiting, done.
        let mut state: HashMap<String, u8> = HashMap::new();
        for n in &graph.nodes { state.insert(n.clone(), 0); }
        fn visit(node: &str, edges: &HashMap<String, Vec<(String, i64)>>, state: &mut HashMap<String, u8>) -> bool {
            if let Some(&s) = state.get(node) {
                if s == 1 { return true; }  // ciclo
                if s == 2 { return false; }
            }
            state.insert(node.to_string(), 1);
            if let Some(nbrs) = edges.get(node) {
                for (v, _) in nbrs {
                    if visit(v, edges, state) { return true; }
                }
            }
            state.insert(node.to_string(), 2);
            false
        }
        for n in &graph.nodes {
            if state[n] == 0 && visit(n, &graph.edges, &mut state) { return Ok(true); }
        }
        Ok(false)
    } else {
        // BFS/DFS tracking parent — si visitamos un nodo ya visitado
        // que no es el parent, hay ciclo.
        let mut visited: BTreeSet<String> = BTreeSet::new();
        for start in &graph.nodes {
            if visited.contains(start) { continue; }
            let mut queue: VecDeque<(String, String)> = VecDeque::new();
            queue.push_back((start.clone(), String::new()));
            while let Some((n, parent)) = queue.pop_front() {
                if visited.contains(&n) { return Ok(true); }
                visited.insert(n.clone());
                if let Some(nbrs) = graph.edges.get(&n) {
                    for (v, _) in nbrs {
                        if v != &parent { queue.push_back((v.clone(), n.clone())); }
                    }
                }
            }
        }
        Ok(false)
    }
}

pub fn graph_nodes(h: u64) -> Result<Vec<String>, String> {
    let g = graphs().lock().unwrap();
    let graph = g.get(&handle_key(h)).ok_or_else(|| format!("unknown graph {h}"))?;
    Ok(graph.nodes.iter().cloned().collect())
}

pub fn graph_drop(h: u64) -> bool { graphs().lock().unwrap().remove(&handle_key(h)).is_some() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_basics() {
        let s = set_new();
        assert!(set_add(s, "a".into()).unwrap());
        assert!(!set_add(s, "a".into()).unwrap());
        assert_eq!(set_len(s).unwrap(), 1);
        assert!(set_contains(s, "a").unwrap());
    }

    #[test]
    fn set_union_intersect() {
        let a = set_from(vec!["1".into(), "2".into(), "3".into()]);
        let b = set_from(vec!["2".into(), "3".into(), "4".into()]);
        let u = set_union(a, b).unwrap();
        let i = set_intersect(a, b).unwrap();
        assert_eq!(set_len(u).unwrap(), 4);
        assert_eq!(set_len(i).unwrap(), 2);
    }

    #[test]
    fn pq_min_returns_smallest_first() {
        let pq = pq_new_min();
        pq_push(pq, "b".into(), 5).unwrap();
        pq_push(pq, "a".into(), 1).unwrap();
        pq_push(pq, "c".into(), 3).unwrap();
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "a");
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "c");
        assert_eq!(pq_pop(pq).unwrap().unwrap(), "b");
    }

    #[test]
    fn counter_most_common_sorted() {
        let c = counter_from(vec!["a".into(),"b".into(),"a".into(),"c".into(),"a".into(),"b".into()]);
        let top = counter_most_common(c, 2).unwrap();
        assert_eq!(top[0], ("a".into(), 3));
        assert_eq!(top[1], ("b".into(), 2));
    }

    #[test]
    fn graph_dijkstra_finds_shortest() {
        let g = graph_new(false);
        graph_add_edge(g, "a".into(), "b".into(), 1).unwrap();
        graph_add_edge(g, "b".into(), "c".into(), 1).unwrap();
        graph_add_edge(g, "a".into(), "c".into(), 5).unwrap();
        let path = graph_shortest_path(g, "a", "c").unwrap();
        assert_eq!(path, vec!["a", "b", "c"]);
    }

    #[test]
    fn graph_toposort_valid() {
        let g = graph_new(true);
        graph_add_edge(g, "a".into(), "b".into(), 0).unwrap();
        graph_add_edge(g, "b".into(), "c".into(), 0).unwrap();
        graph_add_edge(g, "a".into(), "c".into(), 0).unwrap();
        let order = graph_topological_sort(g).unwrap();
        assert_eq!(order[0], "a");
        assert_eq!(order[order.len() - 1], "c");
    }

    #[test]
    fn graph_cycle_detected() {
        let g = graph_new(true);
        graph_add_edge(g, "a".into(), "b".into(), 0).unwrap();
        graph_add_edge(g, "b".into(), "a".into(), 0).unwrap();
        assert!(graph_has_cycle(g).unwrap());
    }
}
