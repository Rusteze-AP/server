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
        // Check if the packet is for this server
        if !check_packet_dest(&packet.routing_header, self.id, &self.logger) {
            self.logger.log_info(&format!("Packet: {:?}", packet));
            return;
        }

        self.logger.log_info(&format!("Received: {:?}", packet));

        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                self.fragment_handler(packet, frag);
            }
            PacketType::FloodResponse(flood_res) => {
                self.routing_handler.update_graph(flood_res.clone());
            }
            PacketType::FloodRequest(flood_req) => {
                self.handle_flood_request(flood_req);
            }
            PacketType::Ack(ack) => {
                self.ack_handler(packet.session_id, ack.fragment_index);
            }
            PacketType::Nack(nack) => {
                self.nack_handler(nack, packet.session_id);
            }
        }
    }
}
