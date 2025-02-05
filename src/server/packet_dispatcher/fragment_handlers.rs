mod chunk_req_handlers;
mod tracker_handlers;

use super::Server;

use packet_forge::MessageType;
use wg_internal::{
    network::SourceRoutingHeader,
    packet::{Fragment, Packet},
};

impl Server {
    /// Call the correct function for the received `MessageType`
    fn message_handler(&mut self, message: &MessageType, addressee_srh: &SourceRoutingHeader) {
        self.logger.log_info(&format!("Processing {message:?}"));
        match message {
            MessageType::SubscribeClient(msg) => {
                self.subscribe_client(msg, addressee_srh);
            }
            MessageType::UpdateFileList(msg) => {
                self.update_file_list(msg);
            }
            MessageType::RequestFileList(msg) => {
                self.send_file_list(msg.client_id, addressee_srh);
            }
            MessageType::RequestPeerList(msg) => {
                self.send_peer_list(msg, addressee_srh);
            }
            MessageType::UnsubscribeClient(msg) => {
                self.unsubscribe_client(msg);
            }
            MessageType::ChunkRequest(msg) => {
                self.handle_chunk_request(msg, addressee_srh);
            }
            _ => {
                self.logger
                    .log_error(&format!("Unexpected message type received: {message:#?}"));
            }
        }
    }

    /// Add the `Fragment` to the map and process it when all the fragments have been received
    pub(crate) fn fragment_handler(&mut self, packet: &Packet, frag: &Fragment) {
        let client_id = packet.routing_header.hops[0];
        let key = (client_id, packet.session_id);

        // Save fragment
        let total_fragments = frag.total_n_fragments;
        self.recv_fragments_map
            .entry(key)
            .or_default()
            .push(frag.clone());

        // Send Ack back to the Client
        self.send_ack(packet, frag.fragment_index);

        // If all fragments are received, assemble the message
        let mut fragments = self.recv_fragments_map.get(&key).unwrap().clone();
        if fragments.len() as u64 == total_fragments {
            let assembled = match self.packet_forge.assemble_dynamic(&mut fragments) {
                Ok(message) => message,
                Err(e) => {
                    self.logger
                        .log_error(&format!("An error occurred when assembling fragments: {e}"));
                    return;
                }
            };

            let mut addressee_srh = packet.routing_header.get_reversed();
            addressee_srh.increase_hop_index();
            self.message_handler(&assembled, &addressee_srh);
        }
    }
}
