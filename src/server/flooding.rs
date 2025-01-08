use std::vec;

use super::Server;

use wg_internal::controller::DroneEvent;
use wg_internal::network::{NodeId, SourceRoutingHeader};
use wg_internal::packet::{FloodRequest, NodeType, Packet};

use crate::packet_send::{get_sender, sc_send_packet, send_packet};
use crate::utils::get_packet_type;

impl Server {
    pub(crate) fn get_flood_id(&mut self) -> u64 {
        self.flood_id += 1;
        self.flood_id
    }

    pub(crate) fn init_flood_request(&mut self) {
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
                self.logger.log_error(&format!(
                    "[SERVER-{}][FLOODING] Sending to [DRONE-{}]: {}",
                    self.id, id, err
                ));
            }
            let packet_str = get_packet_type(&packet.pack_type);
            self.event_dispatcher(&packet, &packet_str);
        }
    }

    fn build_flood_response(flood_req: &FloodRequest) -> (NodeId, Packet) {
        let mut packet = flood_req.generate_response(1); // Note: returns with hop_index = 0;
        packet.routing_header.increase_hop_index();
        let dest = packet.routing_header.current_hop();

        if dest.is_none() {
            return (0, packet);
        }

        (dest.unwrap(), packet)
    }

    fn send_flood_response(&self, sender: NodeId, packet: &Packet) -> Result<(), String> {
        let sender = get_sender(sender, &self.packet_send);

        if let Err(err) = sender {
            return Err(format!(
                "[SERVER-{}][FLOOD RESPONSE] - Error occurred while sending flood response: {}",
                self.id, err
            ));
        }

        let sender = sender.unwrap();
        if let Err(err) = send_packet(&sender, packet) {
            self.logger.log_warn(&format!("[SERVER-{}][FLOOD RESPONSE] - Failed to forward packet to [DRONE-{}]. \n Error: {} \n Trying to use SC shortcut...", self.id, packet.routing_header.current_hop().unwrap(), err));
            // Send to SC
            let res = sc_send_packet(
                &self.controller_send,
                &DroneEvent::ControllerShortcut(packet.clone()),
            );

            if let Err(err) = res {
                self.logger
                    .log_error(&format!("[SERVER-{}][FLOOD RESPONSE] - {}", self.id, err));
                return Err(format!(
                    "[SERVER-{}][FLOOD RESPONSE] - Unable to forward packet to neither next hop nor SC. \n Packet: {}",
                    self.id, packet
                ));
            }

            self.logger.log_debug(&format!("[SERVER-{}][FLOOD RESPONSE] - Successfully sent flood response through SC. Packet: {}", self.id, packet));
        }
        Ok(())
    }

    pub(crate) fn handle_flood_request(&self, message: &FloodRequest) {
        let (dest, packet) = Self::build_flood_response(message);

        let res = self.send_flood_response(dest, &packet);

        if let Err(msg) = res {
            self.logger.log_error(msg.as_str());
        }
    }
}
