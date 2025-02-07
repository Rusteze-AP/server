use super::Server;

use packet_forge::SessionIdT;
use wg_internal::{
    network::NodeId,
    packet::{Nack, NackType, Packet},
};

impl Server {
    /// This function retransmit the packet for which the server received the Nack and tries to calculate a new optimal path.
    fn retransmit_packet(
        &mut self,
        packet: &mut Packet,
        fragment_index: u64,
        session_id: SessionIdT,
    ) {
        let dest = packet.routing_header.hops[packet.routing_header.hops.len() - 1];

        let old_srh = packet.routing_header.clone();

        // Retrieve new best path from server to client, otherwise use the old_one
        let srh = if let Some(new_srh) = self.get_path(self.id, dest) {
            new_srh
        } else {
            self.logger.log_error("[RETRANSMIT PACKET] An error occurred: failed to get routing path, using old routing header");
            old_srh
        };

        let next_hop = srh.hops[srh.hop_index];
        // Assign the new SourceRoutingHeader
        packet.routing_header = srh;

        if let Err(msg) = self.send_packets_vec(&[packet.clone()], next_hop) {
            self.logger.log_error(&msg);
            return;
        }

        self.logger.log_info(&format!(
            "[RETRANSMIT PACKET] Successfully sent packet [ ({fragment_index}, {session_id}) ]"
        ));
    }

    /// Handle different types of nacks
    pub(crate) fn nack_handler(
        &mut self,
        message: &Nack,
        session_id: SessionIdT,
        source_node_id: NodeId,
    ) {
        // Retrieve the packet that generated the nack
        let Some(mut packet) = self
            .sent_fragments_history
            .get(&(message.fragment_index, session_id))
            .cloned()
        else {
            self.logger.log_error(&format!(
                "[NACK] Failed to retrieve packet with [ ({}, {}) ] key from packet history",
                message.fragment_index, session_id
            ));
            return;
        };

        match message.nack_type {
            NackType::Dropped => {
                // Update graph heuristic
                self.routing_handler.node_nack(source_node_id);
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::DestinationIsDrone => {
                self.logger
                    .log_warn(&format!("[NACK] Received DestinationIsDrone for {packet} "));
            }
            NackType::ErrorInRouting(node) => {
                self.logger.log_warn(&format!(
                    "[NACK] Received ErrorInRouting at [NODE-{node}] for {packet}"
                ));
                // Start new flooding
                self.init_flood_request();
                // TODO improve heuristic
                //thread::sleep(Duration::from_secs(4));
                // Retransmit packet
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::UnexpectedRecipient(node) => {
                self.logger.log_warn(&format!(
                    "[NACK] Received UnexpectedRecipient at [NODE-{node}] for {packet}"
                ));
            }
        }
    }
}
