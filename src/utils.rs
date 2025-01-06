use logger::Logger;
use wg_internal::{
    network::{NodeId, SourceRoutingHeader},
    packet::PacketType,
};

/// Check if `routing_header` last hop is the current server (`node_id`) otherwise log a warning.
pub fn check_packet_dest(
    routing_header: &SourceRoutingHeader,
    node_id: NodeId,
    logger: &Logger,
) -> bool {
    if routing_header.hops.last() == Some(&node_id) {
        true
    } else {
        logger.log_warn(&format!(
            "[SERVER-{node_id}] Received a packet with destination: {routing_header:?}",
        ));
        false
    }
}

/// Returns the `PacketType` formatted as as `String`
pub fn get_packet_type(pt: &PacketType) -> String {
    match pt {
        PacketType::Ack(_) => "Ack".to_string(),
        PacketType::Nack(_) => "Nack".to_string(),
        PacketType::FloodRequest(_) => "Flood request".to_string(),
        PacketType::FloodResponse(_) => "Flood response".to_string(),
        PacketType::MsgFragment(_) => "Fragment".to_string(),
    }
}
