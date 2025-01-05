use logger::Logger;
use wg_internal::network::{NodeId, SourceRoutingHeader};

/// Check if `routing_header` last hop is the current server (`node_id`) otherwise log a warning.
pub fn check_packet_dest(
    routing_header: &SourceRoutingHeader,
    node_id: NodeId,
    logger: &Logger,
) -> bool {
    if routing_header.hops.last() == Some(&node_id) {
        true
    } else {
        logger.log_warn(
            format!("[SERVER-{node_id}] Received a packet with destination: {routing_header:?}",)
                .as_str(),
        );
        false
    }
}
