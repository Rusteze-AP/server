use super::Server;

use packet_forge::SessionIdT;
use wg_internal::packet::Packet;

impl Server {
    /// Builds and sends an `Ack` to the `next_hop`. If it fails it tries to use the Simulation Controller
    pub(crate) fn send_ack(&mut self, packet: &Packet, fragment_index: u64) {
        // Dest is 0 because the srh has not been reversed yet
        let dest = packet.routing_header.hops[0];

        let source_routing_header = if let Some(new_srh) = self.get_path(self.id, dest) {
            new_srh
        } else {
            self.logger
                             .log_error("[RETRANSMIT PACKET] An error occurred: failed to get routing path, using old routing header");
            let mut srh = packet.routing_header.get_reversed();
            srh.increase_hop_index();
            srh
        };

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
        }
    }

    /// Pop the corresponding fragment from `packet_history`
    pub(crate) fn ack_handler(&mut self, fragment_index: u64, session_id: SessionIdT) {
        let Some(entry) = self
            .sent_fragments_history
            .remove(&(fragment_index, session_id))
        else {
            self.logger.log_error(&format!(
                "Failed to remove [ ({fragment_index}, {session_id}) ] key from sent fragments history"
            ));
            return;
        };
        self.logger
            .log_debug(&format!("Packet history updated, removed {entry}"));
    }
}
