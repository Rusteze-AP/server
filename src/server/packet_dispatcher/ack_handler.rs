use crate::packet_send::sc_send_packet;

use super::Server;

use packet_forge::SessionIdT;
use wg_internal::{controller::DroneEvent, packet::Packet};

impl Server {
    /// Builds and sends an `Ack` to the `next_hop`. If it fails it tries to use the Simulation Controller
    pub(crate) fn send_ack(&mut self, packet: &Packet, fragment_index: u64) {
        let source_routing_header = packet.routing_header.get_reversed();
        if source_routing_header.hop_index != 1 {
            self.logger.log_error(&format!(
                "Unable to reverse source routing header. \n Hops: {} \n Hop index: {}",
                packet.routing_header, packet.routing_header.hop_index
            ));
            return;
        }
        let next_hop = source_routing_header.hops[1];
        let ack = Packet::new_ack(source_routing_header, packet.session_id, fragment_index);

        if let Err(msg) = self.send_packets_vec(&[ack], next_hop) {
            self.logger.log_error(&msg);
            self.logger
                .log_debug(&format!("[ACK] Trying to use SC shortcut..."));

            // Send to SC
            if let Err(msg) = sc_send_packet(
                &self.controller_send,
                &DroneEvent::ControllerShortcut(packet.clone()),
            ) {
                self.logger.log_error(&format!("[ACK] - {}", msg));
                self.logger.log_error(&format!(
                    "[ACK] - Unable to forward packet to neither next hop nor SC. \n Packet: {}",
                    packet
                ));
                return;
            }

            self.logger.log_debug(&format!(
                "[ACK] - Successfully sent flood response through SC. Packet: {}",
                packet
            ));
        }
    }

    /// Pop the corresponding fragment from `packet_history`
    pub(crate) fn ack_handler(&mut self, fragment_index: u64, session_id: SessionIdT) {
        let Some(entry) = self
            .sent_fragments_history
            .remove(&(fragment_index, session_id))
        else {
            self.logger.log_error(&format!(
                "Failed to remove [ ({}, {}) ] key from packet history",
                fragment_index, session_id
            ));
            return;
        };
        self.logger
            .log_info(&format!("Packet history updated, removed: {:?}", entry));
    }
}
