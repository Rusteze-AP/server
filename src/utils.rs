use wg_internal::network::{NodeId, SourceRoutingHeader};

pub fn check_packet_dest(routing_header: &SourceRoutingHeader, node_id: NodeId) -> bool {
    routing_header.hops.last() == Some(&node_id)
}
