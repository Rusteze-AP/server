use super::Server;

use packet_forge::SessionIdT;
use wg_internal::packet::{Nack, NackType, Packet};

impl Server {
    /// This function retransmit the packet for which the server received the Nack and tries to calculate a new optimal path.
    fn retransmit_packet(
        &mut self,
        packet: &mut Packet,
        fragment_index: u64,
        session_id: SessionIdT,
    ) {
        let dest = packet.routing_header.hops[packet.routing_header.hops.len()];

        // Retrieve new best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, dest) else {
            self.logger
                .log_error("An error occurred: failed to get routing path");
            return;
        };

        let next_hop = srh.hops[srh.hop_index];
        // Assign the new SourceRoutingHeader
        packet.routing_header = srh;

        if let Err(msg) = self.send_packets_vec(&[packet.clone()], next_hop) {
            self.logger.log_error(&msg);
            return;
        }

        self.logger.log_info(&format!(
            "Successfully re-sent packet [ ({}, {}) ]",
            fragment_index, session_id
        ));
    }

    /// Handle different types of nacks
    pub(crate) fn nack_handler(&mut self, message: &Nack, session_id: SessionIdT) {
        // Retrieve the packet that generated the nack
        let Some(mut packet) = self
            .packets_history
            .get(&(message.fragment_index, session_id))
            .cloned()
        else {
            self.logger.log_error(&format!(
                "Failed to retrieve packet with [ ({}, {}) ] key from packet history",
                message.fragment_index, session_id
            ));
            return;
        };

        match message.nack_type {
            NackType::Dropped => {
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::DestinationIsDrone => {
                self.logger
                    .log_warn(&format!("Received DestinationIsDrone for {:?} ", packet));
            }
            NackType::ErrorInRouting(node) => {
                self.logger.log_warn(&format!(
                    "Received ErrorInRouting at [NODE-{}] for {}",
                    node, packet
                ));
                // Start new flooding
                // TODO change euristic
                self.init_flood_request();
                // Retransmit packet
                self.retransmit_packet(&mut packet, message.fragment_index, session_id);
            }
            NackType::UnexpectedRecipient(node) => {
                self.logger.log_warn(&format!(
                    "Received UnexpectedRecipient at [NODE-{}] for {}",
                    node, packet
                ));
            }
        }
    }
}
