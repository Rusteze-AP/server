use super::Server;

use wg_internal::network::SourceRoutingHeader;
use wg_internal::packet::{FloodRequest, Packet};

use crate::packet_send::send_packet;

impl Server {
    pub(crate) fn get_flood_id(&mut self) -> u64 {
        self.flood_id += 1;
        self.flood_id
    }

    pub(crate) fn init_flood_request(&mut self) {
        let flood_req = FloodRequest::new(self.get_flood_id(), self.id);
        for (id, sender) in &self.packet_send {
            let packet = Packet::new_flood_request(
                SourceRoutingHeader::new(vec![], 0),
                self.packet_forge.get_session_id(),
                flood_req.clone(),
            );
            if let Err(err) = send_packet(sender, &packet) {
                self.logger.log_error(
                    format!(
                        "[SERVER-{}][FLOODING] Sending to [DRONE-{}]: {}",
                        self.id, id, err
                    )
                    .as_str(),
                );
            }
        }
    }
}
