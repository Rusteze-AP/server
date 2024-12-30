use super::Server;

use packet_forge::MessageType;
use wg_internal::packet::{Packet, PacketType};

impl Server {
    pub(crate) fn handle_message(&mut self, message: MessageType) {
        match message {
            MessageType::SubscribeClient(client) => {},
            MessageType::UpdateFileList(files) => {},
            MessageType::RequestFileList(files) => {},
            MessageType::RequestPeerList(file) => {},
            MessageType::UnsubscribeClient(client) => {},
            _ => {
                self.logger.log_error(format!("Unexpected message type received: {:#?}", message).as_str());
            }
        }
    }

    pub(crate) fn packet_dispatcher(&mut self, packet: &Packet) {
        let key = (packet.routing_header.hops[0], packet.session_id);
        match &packet.pack_type {
            PacketType::MsgFragment(frag) => {
                // Save fragment
                let total_fragments = frag.total_n_fragments;   
                self.packets_map.entry(key).or_default().push(frag.clone());

                // If all fragments are received, assemble the message
                let fragments = self.packets_map.get(&key).unwrap();
                if fragments.len() == total_fragments as usize {
                    let assembled = match self.packet_forge.assemble_dynamic(fragments.clone()) {
                        Ok(message) => message,
                        Err(e) => panic!("Error: {e}"),
                    };

                    self.handle_message(assembled);
                }
            }
            _ => {
                println!("Server {} received a packet: {:?}", self.id, packet);
            }
        }
    }
}