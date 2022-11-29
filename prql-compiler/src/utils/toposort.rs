use std::collections::HashMap;

type Dag = Vec<Vec<usize>>;

struct Toposort {
    nodes: Vec<Node>,
    order: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Node {
    visiting: bool,
    done: bool,
}

pub fn toposort<Key: Eq + std::hash::Hash + Clone>(
    dependencies: &[(Key, Vec<Key>)],
) -> Option<Vec<&Key>> {
    // create mapping from Key to usize
    let index: HashMap<&Key, usize> = dependencies
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (key, index))
        .collect();

    // map DAG from Key to usize
    let dag: Dag = dependencies
        .iter()
        .map(|(_, deps)| deps.iter().map(|d| index[d]).collect())
        .collect();

    // init toposort
    let empty = Node {
        visiting: false,
        done: false,
    };
    let mut toposort = Toposort {
        nodes: vec![empty; index.len()],
        order: Vec::with_capacity(index.len()),
    };

    // start visits
    while toposort.order.len() < dependencies.len() {
        for start_at in 0..index.len() {
            toposort.visit(&dag, start_at).ok()?;
        }
    }

    // unmap
    Some(toposort.order.iter().map(|i| &dependencies[*i].0).collect())
}

impl Toposort {
    fn visit(&mut self, dag: &Dag, n: usize) -> Result<(), ()> {
        let node = self.nodes.get_mut(n).unwrap();
        if node.done {
            return Ok(());
        }
        if node.visiting {
            return Err(());
        }
        node.visiting = true;

        for m in &dag[n] {
            self.visit(dag, *m)?;
        }

        let node = self.nodes.get_mut(n).unwrap();
        node.visiting = false;
        node.done = true;
        self.order.push(n);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::toposort;

    #[test]
    fn normal_sort() {
        let dependencies = vec![
            ("a", vec!["b"]),
            ("b", vec!["c"]),
            ("c", vec![]),
            ("d", vec![]),
        ];
        let order = toposort(&dependencies).unwrap();

        let order = order.into_iter().map(|s| *s).collect_vec();
        assert_eq!(order, vec!["c", "b", "a", "d"]);
    }

    #[test]
    fn normal_sort_2() {
        let dependencies = vec![
            ("a", vec![]),
            ("b", vec![]),
            ("c", vec!["b"]),
            ("d", vec!["c"]),
        ];
        let order = toposort(&dependencies).unwrap();

        let order = order.into_iter().map(|s| *s).collect_vec();
        assert_eq!(order, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn dag_with_cycle() {
        let dependencies = vec![
            ("a", vec!["b"]),
            ("b", vec!["c", "d"]),
            ("c", vec![]),
            ("d", vec!["a"]),
        ];
        let order = toposort(&dependencies);

        assert!(order.is_none());
    }

    #[test]
    fn parallel_when_ambiguous() {
        let dependencies = vec![
            ("a", vec!["b"]),
            ("b", vec![]),
            ("c", vec!["b"]),
            ("d", vec!["b"]),
        ];

        let order = toposort(&dependencies).unwrap();

        let order = order.into_iter().map(|s| *s).collect_vec();
        assert_eq!(order, vec!["b", "a", "c", "d"]);
    }
}
