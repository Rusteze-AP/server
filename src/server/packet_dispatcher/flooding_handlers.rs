use packet_forge::SessionIdT;
use rand::Rng;
use std::time::Instant;
use std::vec;

use super::Server;

use wg_internal::network::{NodeId, SourceRoutingHeader};
use wg_internal::packet::{FloodRequest, NodeType, Packet};

use crate::packet_send::send_packet;
use crate::utils::get_packet_type;

impl Server {
    fn get_flood_id(&mut self) -> u64 {
        let mut rng = rand::rng();

        // Generate a random u64
        let mut random_number: u64 = rng.random();
        while !self.used_flood_id.insert(random_number) {
            random_number = rng.random();
        }

        self.curr_flood_id = random_number;
        self.curr_flood_id
    }

    pub(crate) fn init_flood_request(&mut self) {
        // Reset flooding countdown
        self.flood_countdown = Instant::now();

        self.logger.log_info("Initiating flooding...");

        let flood_req = FloodRequest {
            flood_id: self.get_flood_id(),
            initiator_id: self.id,
            path_trace: vec![(self.id, NodeType::Server)],
        };
        for (id, sender) in &self.packet_send {
            let packet = Packet::new_flood_request(
                SourceRoutingHeader::new(vec![], 0),
                self.packet_forge.get_session_id(),
                flood_req.clone(),
            );
            if let Err(err) = send_packet(sender, &packet) {
                self.logger
                    .log_error(&format!("[FLOODING] Sending to [DRONE-{}]: {}", id, err));
            }
            let packet_str = get_packet_type(&packet.pack_type);
            self.event_dispatcher(&packet, &packet_str);
        }
    }

    fn build_flood_response(
        &self,
        flood_req: &FloodRequest,
        session_id: SessionIdT,
    ) -> (NodeId, Packet) {
        let mut flood_req = flood_req.clone();
        flood_req.path_trace.push((self.id, NodeType::Server));

        let mut packet = flood_req.generate_response(session_id); // Note: returns with hop_index = 0;
        packet.routing_header.increase_hop_index();
        let dest = packet.routing_header.current_hop();

        if dest.is_none() {
            return (0, packet);
        }

        (dest.unwrap(), packet)
    }

    fn send_flood_response(&self, next_hop: NodeId, packet: &Packet) -> Result<(), String> {
        self.send_packets_vec(&[packet.clone()], next_hop)
    }

    /// Build a flood response for the received flood request
    pub(crate) fn handle_flood_request(&self, message: &FloodRequest, session_id: SessionIdT) {
        let (dest, packet) = self.build_flood_response(message, session_id);

        let res = self.send_flood_response(dest, &packet);

        if let Err(msg) = res {
            self.logger.log_error(&msg);
        }
    }
}
