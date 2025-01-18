mod ack_handler;
mod flooding_handlers;
mod fragment_handlers;
mod nack_handler;

use std::fmt::Debug;

use super::Server;

use crate::utils::check_packet_dest;
use packet_forge::Metadata;
use wg_internal::packet::{Packet, PacketType};

impl Server {
    // Call the correct function for the received `Packet`
    pub(crate) fn packet_dispatcher<M: Metadata + Debug>(&mut self, packet: &Packet) {
        // Check if the packet is for this server
        if !check_packet_dest(&packet.routing_header, self.id, &self.logger) {
            self.logger
                .log_info(format!("[SERVER-{}] Packet: {:?}", self.id, packet).as_str());
            return;
        }

        self.logger
            .log_info(format!("[SERVER-{}] Received: {:?}", self.id, packet).as_str());

        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                self.fragment_handler::<M>(packet, frag);
            }
            PacketType::FloodResponse(flood_res) => {
                // Update graph with flood response
                self.routing_handler.update_graph(flood_res.clone());
            }
            PacketType::FloodRequest(flood_req) => {
                // Build flood response
                self.handle_flood_request(flood_req);
            }
            PacketType::Ack(ack) => {
                self.ack_handler(packet.session_id, ack.fragment_index);
            }
            PacketType::Nack(nack) => {
                // Handle different nacks
                self.nack_handler(nack, packet.session_id);
            }
        }
    }
}
