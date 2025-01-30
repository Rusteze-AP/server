mod ack_handler;
mod flooding_handlers;
mod fragment_handlers;
mod nack_handler;

use super::Server;

use crate::utils::check_packet_dest;
use wg_internal::packet::{Packet, PacketType};

impl Server {
    /// Call the correct function for the received `Packet`
    pub(crate) fn packet_dispatcher(&mut self, packet: &Packet) {
        self.logger.log_info(&format!("Received: {packet}"));

        // Handle flood request since SRH is empty
        if let PacketType::FloodRequest(flood_req) = &packet.pack_type {
            self.handle_flood_request(flood_req, packet.session_id);
            return;
        }

        // Check if the packet is for this server
        if !check_packet_dest(&packet.routing_header, self.id, &self.logger) {
            self.logger
                .log_warn(&format!("Packet has wrong destination!"));
            return;
        }

        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                self.fragment_handler(packet, frag);
            }
            PacketType::FloodResponse(flood_res) => {
                self.routing_handler.update_graph(flood_res.clone());
            }
            PacketType::Ack(ack) => {
                self.ack_handler(ack.fragment_index, packet.session_id);
            }
            PacketType::Nack(nack) => {
                self.nack_handler(nack, packet.session_id);
            }
            PacketType::FloodRequest(flood_req) => {
                self.logger.log_error(&format!(
                    "Packet reached not handled match case: {flood_req}"
                ));
            }
        }
    }
}
