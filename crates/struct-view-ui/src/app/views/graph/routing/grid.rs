use super::*;
#[cfg(test)]
use struct_view_routing::orthogonal::simplify_route;
use struct_view_routing::orthogonal::{OrthogonalRouter, OrthogonalRouterOptions, Point, Size};

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
        Self::new_for_graph(node_positions, &routed_nodes)
    }

    pub(in crate::app::views::graph) fn new_for_graph(
        node_positions: &[Pos2],
        routed_nodes: &[usize],
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

    #[cfg(test)]
    pub(in crate::app::views) fn route_edge(&self, source: usize, target: usize) -> Vec<Pos2> {
        let ports = graph_edge_ports(&self.node_positions, &[(source, target)])[0];
        self.route_edge_with_ports(source, target, ports, &[])
    }

    pub(in crate::app::views) fn route_edge_with_ports(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edges: &[Vec<Pos2>],
    ) -> Vec<Pos2> {
        let route_index = GraphRouteSegmentIndex::new(routed_edges);
        self.route_edge_with_index(source, target, edge_ports, &route_index)
    }

    pub(super) fn route_edge_with_index(
        &self,
        source: usize,
        target: usize,
        edge_ports: GraphEdgePorts,
        routed_edge_index: &GraphRouteSegmentIndex,
    ) -> Vec<Pos2> {
        self.inner
            .route_edge(source, target, edge_ports, &routed_edge_index.inner)
            .expect("the UI graph router supplies valid endpoints and route geometry")
            .into_iter()
            .map(|point| Pos2::new(point.x, point.y))
            .collect()
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
