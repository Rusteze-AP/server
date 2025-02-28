use crate::{
    packet_send::{get_sender, sc_send_packet, send_packet},
    utils::get_packet_type,
};

use super::Server;

use packet_forge::{FileHash, Metadata};
use wg_internal::{
    controller::DroneEvent,
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType},
};

impl Server {
    /// Check if the received `file_hash` is correct.
    /// ### Error
    /// Log the two mismatched hash
    pub(crate) fn check_hash<M: Metadata>(
        file_hash: FileHash,
        file_metadata: &M,
    ) -> Result<(), String> {
        if file_hash != file_metadata.compact_hash_u16() {
            return Err(format!(
                "File hash mismatch! Received: [ {:?} ]\nCalculated: [ {:?} ]",
                file_hash,
                file_metadata.compact_hash_u16()
            ));
        }
        Ok(())
    }

    /// Insert a vector of packets inside the packets sent history
    /// ### Error
    /// If a `Packet` inside the vector does not contain a `Fragment` it logs an error.
    pub(crate) fn insert_packet_history(&mut self, packets: &[Packet]) {
        for p in packets {
            let PacketType::MsgFragment(fragment) = &p.pack_type else {
                self.logger.log_error(&format!(
                    "Found {:?} while saving Fragments to packet_history! Ignoring.",
                    p.pack_type
                ));
                continue;
            };

            let key = (fragment.fragment_index, p.session_id);
            self.sent_fragments_history.insert(key, p.clone());
        }
    }

    /// Retrieve the best path from-to and log error if the path cannot be found.
    pub(crate) fn get_path(&mut self, from: NodeId, to: NodeId) -> Option<SourceRoutingHeader> {
        if let Some(srh) = self.routing_handler.best_path(from, to) {
            Some(srh)
        } else {
            self.logger
                .log_error(&format!("No path found from {} to {}!", self.id, to));
            None
        }
    }

    /// Sends a `DroneEvent` containing the `packet` that has been sent.
    pub(crate) fn event_dispatcher(&self, packet: &Packet, packet_str: &str) {
        if let Err(err) = sc_send_packet(
            &self.controller_send,
            &DroneEvent::PacketSent(packet.clone()),
        ) {
            self.logger.log_error(&format!(
                "[{}] - Packet event forward: {}",
                packet_str.to_ascii_uppercase(),
                err
            ));
            return;
        }
        self.logger.log_debug(&format!(
            "[{}] - Packet event sent successfully",
            packet_str.to_ascii_uppercase()
        ));
    }

    /// Takes a vector of packets and sends them to the `next_hop`
    pub(crate) fn send_packets_vec(
        &self,
        packets: &[Packet],
        next_hop: NodeId,
    ) -> Result<(), String> {
        // Get the sender channel for the next hop and forward
        let sender = get_sender(next_hop, &self.packet_send)?;

        for packet in packets {
            let packet_str = get_packet_type(&packet.pack_type).to_uppercase();
            if let Err(err) = send_packet(&sender, packet) {
                // If Packet is ack, nack or flood response try controller shortcut
                if packet_str == "ACK" || packet_str == "NACK" || packet_str == "FLOOD RESPONSE" {
                    self.logger.log_warn(&format!("[{packet_str}] - Failed to forward packet to [DRONE-{next_hop}]. \n Error: {err} \n Trying to use SC shortcut..."));
                    // Send to SC
                    let res = sc_send_packet(
                        &self.controller_send,
                        &DroneEvent::ControllerShortcut(packet.clone()),
                    );

                    if let Err(err) = res {
                        self.logger.log_error(&format!("[{packet_str}] - {err}"));
                        return Err(format!(
                    "[{packet_str}] - Unable to forward packet to neither next hop nor SC. \n {packet}"));
                    }

                    self.logger
                        .log_debug(&format!("[{packet_str}] - Sent through SC: {packet}",));

                    return Ok(());
                }

                return Err(format!(
                    "Failed to send packet to [DRONE-{next_hop}].\n {packet} \n Error: {err}"
                ));
            }

            self.logger
                .log_debug(&format!("[{packet_str}] - Sent {packet}"));
            self.event_dispatcher(packet, &packet_str);
        }
        Ok(())
    }

    /// This function has two purposes:
    /// - send the fragments contained within each Packet to their destination
    /// - save each packet into `packet_history`
    /// ### Error
    /// If the channel of the `next_hop` is not found returns Err(String).
    pub(crate) fn send_save_packets(
        &mut self,
        packets: &[Packet],
        next_hop: NodeId,
    ) -> Result<(), String> {
        self.send_packets_vec(packets, next_hop)?;

        self.insert_packet_history(packets);

        Ok(())
    }
}
