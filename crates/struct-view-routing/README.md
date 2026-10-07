# struct-view-routing

Reusable, dependency-free weighted-graph pathfinding for Rust applications.
Use the included `AdjacencyListGraph` or implement `RoutingGraph` to adapt
an existing graph without moving its data model.

## Algorithms

| Algorithm | Use |
| --- | --- |
| `breadth_first_search` | Minimum-hop paths when edge weights are ignored |
| `dijkstra` | Single-pair shortest paths with non-negative weights |
| `a_star` | Single-pair shortest paths with a caller-supplied admissible heuristic |
| `a_star_indexed` | A* for implicit dense integer-indexed state spaces |
| `bellman_ford` | Single-source paths with negative edges |
| `floyd_warshall` | All-pairs paths with negative edges |
| `yen_k_shortest_paths` | The first `k` loopless paths with non-negative weights |
| `orthogonal::OrthogonalRouter` | Obstacle-aware orthogonal routes between positioned nodes |

Every weighted algorithm rejects non-finite weights. Dijkstra, A*, and Yen
also reject negative weights; Bellman-Ford reports reachable negative cycles,
while Floyd-Warshall reports any negative cycle. Results preserve node order,
directed edge IDs, per-edge costs, and total cost.

## Example

```rust
use struct_view_routing::{AdjacencyListGraph, dijkstra};

let mut graph = AdjacencyListGraph::<&str, &str>::new();
graph.add_node("api");
graph.add_node("database");
graph.add_edge(&"api", &"database", "query", 2.0)?;

let path = dijkstra(&graph, "api", "database")?.expect("path exists");
assert_eq!(path.nodes, ["api", "database"]);
assert_eq!(path.cost, 2.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`AdjacencyListGraph` stores nodes in insertion order, rejects duplicate edge
IDs, and represents undirected edges as two directed arcs with distinct IDs.
Custom graph implementations can expose their own stable node and edge IDs
through the `RoutingGraph` trait.

## Obstacle-aware orthogonal routing

`orthogonal::OrthogonalRouter` handles ports, node obstacles, route indexes,
crossings, shared segments, and indexed A* search. Its geometry types are
independent of UI frameworks, so an application only needs to convert its
node rectangles and positions to `Point`, `Size`, and `Rect`.

Unobstructed connections stay straight, even when they cross other edges.
Otherwise, the selected A* backend minimizes route length plus
`GRAPH_ROUTE_BEND_COST` (48 units) per bend, including turns at port leads, on
the retained routing grid, so a short jog is never taken just to save a few
units of length. Detours keep out of the port-lead band around the route's own
source and target nodes, which prevents loops back across its own port lead;
the band is only entered when no other path exists. Among otherwise equal
routes, the search avoids the straight exits of other reserved ports, so later
edges can leave their ports without extra bends. There are no crossing
penalties and no first-found local-route shortcut.

Shared collinear sections are forbidden, including short overlaps and port
leads. Parallel tracks normally retain the configured edge clearance. If dense
ports leave no path with that clearance, intermediate tracks are added between
existing coordinates and lane spacing is relaxed;
overlapping sections and node obstacles remain forbidden. Incompatible fixed
ports return `RoutingError::NoOrthogonalPath` instead of an overlapping route.
Legacy geometry-scoring helpers remain available independently of route search.
Call `OrthogonalRouter::reserve_port_leads` with all edges and their ports to
reserve every port lead up front: other routes may cross a reserved lead but never
run along it, so edges routed later always find their own port entry free.

The graph UI propagates routing failures with the edge endpoints to the
background-operation error display instead of panicking the calculation worker.
Spacing is local: only the column and row gaps next to a node with more than
eight incident edges grow (parallel edges and both ends of self-loops count).
Each layout pass collects every edge without a free path; the next pass widens
only the gaps around those edges' endpoints (and, for edges that stay crowded,
the gaps between their endpoints) and rebuilds ports and routes. Up to eight
passes run; node card sizes stay unchanged. Only `NoOrthogonalPath` triggers
expansion; invalid inputs are reported directly. After a successful pass, up to
two straightening passes compare every route with the same edge routed alone
and widen the gaps around edges that other routes forced into extra bends or
noticeably longer paths.

```rust
use struct_view_routing::orthogonal::{
    OrthogonalRouter, OrthogonalRouterOptions, Point, RouteIndex, Size, assign_edge_ports,
};

let nodes = [
    Point::new(40.0, 80.0),
    Point::new(260.0, 80.0),
    Point::new(480.0, 80.0),
];
let options = OrthogonalRouterOptions::new(Size::new(60.0, 40.0), Size::new(100.0, 50.0));
let router = OrthogonalRouter::new(&nodes, &[0, 1, 2], options)?;
let ports = assign_edge_ports(&nodes, options.node_size, &[(0, 2)])?;
let accepted_routes = RouteIndex::new(&[], options.edge_clearance)?;
let route = router.route_edge(0, 2, ports[0], &accepted_routes)?;
# Ok::<(), struct_view_routing::RoutingError>(())
```

## Crate structure

```text
struct-view-routing/
├── Cargo.toml
└── src/
    ├── lib.rs       public types, errors, and RoutingGraph interface
    ├── graph.rs     reusable adjacency-list graph
    ├── search.rs    breadth-first search, Dijkstra, and A*
    ├── all_pairs.rs Bellman-Ford and Floyd-Warshall results and algorithms
    ├── yen.rs       K-shortest loopless paths
    ├── tests.rs     graph-model and shortest-path tests
    └── orthogonal/
        ├── mod.rs       orthogonal routing API
        ├── geometry.rs  dependency-free 2D points, vectors, rectangles
        ├── index.rs     route conflict index and geometry scoring
        ├── ports.rs     neighbor-ordered edge-port assignment
        ├── router.rs    obstacle-aware routing grid
        ├── router/
        │   └── search.rs indexed A* search over the routing grid
        └── tests.rs     route geometry and obstacle tests
```
