use super::*;
#[cfg(test)]
use struct_view_routing::orthogonal::simplify_route;
use struct_view_routing::orthogonal::{
    OrthogonalRouter, OrthogonalRouterOptions, Point, RoutingSearchBackend, Size,
};

pub(in crate::app::views) struct GraphRoutingGrid {
    inner: OrthogonalRouter,
    #[cfg(test)]
    pub(super) node_positions: Vec<Pos2>,
    #[cfg(test)]
    pub(super) node_rects: Vec<egui::Rect>,
    #[cfg(test)]
    pub(super) obstacles: Vec<egui::Rect>,
}

impl GraphRoutingGrid {
    #[cfg(test)]
    pub(in crate::app::views) fn new(node_positions: &[Pos2]) -> Self {
        let routed_nodes = (0..node_positions.len()).collect::<Vec<_>>();
        Self::new_for_graph(node_positions, &routed_nodes, RoutingSearchBackend::Builtin)
    }

    pub(in crate::app::views::graph) fn new_for_graph(
        node_positions: &[Pos2],
        routed_nodes: &[usize],
        search_backend: RoutingSearchBackend,
    ) -> Self {
        #[cfg(test)]
        let node_rects = node_positions
            .iter()
            .map(|center| egui::Rect::from_center_size(*center, GRAPH_NODE_SIZE))
            .collect::<Vec<_>>();
        #[cfg(test)]
        let obstacles = node_rects
            .iter()
            .map(|rect| rect.expand(GRAPH_ROUTE_CLEARANCE))
            .collect::<Vec<_>>();
        #[cfg(test)]
        let node_positions = node_positions.to_vec();
        let positions = node_positions
            .iter()
            .map(|point| Point::new(point.x, point.y))
            .collect::<Vec<_>>();
        let mut options = OrthogonalRouterOptions::new(
            Size::new(GRAPH_NODE_SIZE.x, GRAPH_NODE_SIZE.y),
            Size::new(GRAPH_STEP.x, GRAPH_STEP.y),
        );
        options.search_backend = search_backend;
        options.obstacle_clearance = GRAPH_ROUTE_CLEARANCE;
        options.edge_clearance = GRAPH_EDGE_CLEARANCE;
        let inner = OrthogonalRouter::new(&positions, routed_nodes, options)
            .expect("the UI graph layout supplies finite node geometry");

        Self {
            inner,
            #[cfg(test)]
            node_positions: node_positions.to_vec(),
            #[cfg(test)]
            node_rects,
            #[cfg(test)]
            obstacles,
        }
    }

    /// Keeps every edge's port entry free from routes of other edges.
    pub(in crate::app::views::graph) fn reserve_port_leads(
        &mut self,
        endpoints: &[(usize, usize)],
        ports: &[GraphEdgePorts],
    ) {
        self.inner
            .reserve_port_leads(endpoints, ports)
            .expect("the UI graph layout supplies matching edges and ports");
    }

    /// Edges whose routes are worse than they would be without other edges: they need extra
    /// bends or a noticeably longer path, so more space around them would straighten them.
    /// Call this on a grid without reserved port leads.
    pub(in crate::app::views::graph) fn detoured_edges(
        &self,
        endpoints: &[(usize, usize)],
        ports: &[GraphEdgePorts],
        routes: &[Vec<Pos2>],
    ) -> Vec<(usize, usize)> {
        // Parallel lanes of edges between the same nodes may differ by a few lane offsets.
        const LENGTH_TOLERANCE: f32 = 24.0;
        let length = |route: &[Pos2]| {
            route
                .windows(2)
                .map(|pair| pair[0].distance(pair[1]))
                .sum::<f32>()
        };
        let empty = GraphRouteSegmentIndex::new(&[]);
        endpoints
            .iter()
            .zip(ports)
            .zip(routes)
            .filter(|&((&(source, target), &edge_ports), route)| {
                self.route_edge_with_index(source, target, edge_ports, &empty)
                    .is_ok_and(|alone| {
                        route.len() > alone.len()
                            || length(route) > length(&alone) + LENGTH_TOLERANCE
                    })
            })
            .map(|((&endpoints, _), _)| endpoints)
            .collect()
    }

    #[cfg(test)]
    pub(in crate::app::views) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        let ports = graph_edge_ports(&self.node_positions, &[(source, target)])[0];
        self.route_edge_with_ports(source, target, ports, &[])
    }

    #[cfg(test)]
    pub(in crate::app::views) fn route_edge_with_ports(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let route_index = GraphRouteSegmentIndex::new(routed_edges);
        self.route_edge_with_index(source, target, edge_ports, &route_index)
            .expect("test graph must have a valid route")
    }

    pub(super) fn route_edge_with_index(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edge_index: &GraphRouteSegmentIndex,
    ) -> struct_view_routing::RoutingResult<Vec<Pos2>> {
        self.inner
            .route_edge(source, target, edge_ports, &routed_edge_index.inner)
            .map(|route| {
                route
                    .into_iter()
                    .map(|point| Pos2::new(point.x, point.y))
                    .collect()
            })
    }

    #[cfg(test)]
    pub(super) fn simplify_graph_route(points: Vec<Pos2>) -> Vec<Pos2> {
        simplify_route(
            points
                .into_iter()
                .map(|point| Point::new(point.x, point.y))
                .collect(),
        )
        .into_iter()
        .map(|point| Pos2::new(point.x, point.y))
        .collect()
    }
}
