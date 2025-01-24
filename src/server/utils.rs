use crate::{
    packet_send::{get_sender, sc_send_packet, send_packet},
    utils::get_packet_type,
};

use super::Server;

use packet_forge::{FileHash, FileMetadata, Metadata};
use std::collections::HashSet;
use wg_internal::{
    controller::DroneEvent,
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType},
};

impl Server {
    pub(crate) fn log_error(&self, msg: &str) {
        self.logger
            .log_error(&format!("[SERVER-{}] {}", self.id, msg));
    }

    pub(crate) fn log_info(&self, msg: &str) {
        self.logger
            .log_info(&format!("[SERVER-{}] {}", self.id, msg));
    }

    pub(crate) fn log_debug(&self, msg: &str) {
        self.logger
            .log_debug(&format!("[SERVER-{}] {}", self.id, msg));
    }

    pub(crate) fn log_warn(&self, msg: &str) {
        self.logger
            .log_warn(&format!("[SERVER-{}] {}", self.id, msg));
    }

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
                self.log_error(&format!(
                    "Found {:?} while saving Fragments to packet_history! Ignoring.",
                    p.pack_type
                ));
                continue;
            };

            let key = (fragment.fragment_index, p.session_id);
            self.packets_history.insert(key, p.clone());
        }
    }

    /// Retrieve the best path from-to and log error if the path cannot be found.
    pub(crate) fn get_path(&mut self, from: NodeId, to: NodeId) -> Option<SourceRoutingHeader> {
        if let Some(srh) = self.routing_handler.best_path(from, to) {
            Some(srh)
        } else {
            self.log_error(&format!("No path found from {} to {}!", self.id, to));
            None
        }
    }

    /// Sends a `DroneEvent` containing the `packet` that has been sent.
    pub(crate) fn event_dispatcher(&self, packet: &Packet, packet_str: &str) {
        if let Err(err) = sc_send_packet(
            &self.controller_send,
            &DroneEvent::PacketSent(packet.clone()),
        ) {
            self.log_error(&format!(
                "[{}] - Packet event forward: {}",
                packet_str.to_ascii_uppercase(),
                err
            ));
            return;
        }
        self.log_debug(&format!(
            "[{}] - Packet event sent successfully",
            packet_str.to_ascii_uppercase()
        ));
    }

    /// Takes a vector of packets and sends them to the `next_hop`
    pub(crate) fn send_packets_vec(
        &mut self,
        packets: &[Packet],
        next_hop: NodeId,
    ) -> Result<(), String> {
        // Get the sender channel for the next hop and forward
        let sender = get_sender(next_hop, &self.packet_send);
        if sender.is_err() {
            return Err(format!("{}", &sender.unwrap_err()));
        }
        let sender = sender.unwrap();

        for packet in packets {
            let packet_str = get_packet_type(&packet.pack_type);
            // IF PACKET IS ACK, NACK OR FLOOD RESPONSE ADD TRY CONTROLLERSHORTCUT
            if packet_str == "Ack" || packet_str == "Nack" || packet_str == "Flood response" {
                // TODO Send Ack Nack Flood response to SC
            }
            if let Err(err) = send_packet(&sender, packet) {
                return Err(format!(
                    "Failed to send packet to [DRONE-{}].\nPacket: {}\n Error: {}",
                    next_hop, packet, err
                ));
            }
            self.event_dispatcher(packet, &packet_str);
        }
        Ok(())
    }

    /// Builds and sends an `Ack` to the `next_hop`. If it fails it tries to use the Simulation Controller
    pub(crate) fn send_ack(&mut self, packet: &Packet, fragment_index: u64) {
        let source_routing_header = packet.routing_header.get_reversed();
        if source_routing_header.hop_index != 1 {
            self.log_error(&format!(
                "Unable to reverse source routing header. \n Hops: {} \n Hop index: {}",
                packet.routing_header, packet.routing_header.hop_index
            ));
            return;
        }
        let next_hop = source_routing_header.hops[1];
        let ack = Packet::new_ack(source_routing_header, packet.session_id, fragment_index);

        if let Err(msg) = self.send_packets_vec(&[ack], next_hop) {
            self.log_error(&msg);
            self.log_debug(&format!("[ACK] Trying to use SC shortcut..."));

            // Send to SC
            if let Err(msg) = sc_send_packet(
                &self.controller_send,
                &DroneEvent::ControllerShortcut(packet.clone()),
            ) {
                self.log_error(&format!("[ACK] - {}", msg));
                self.log_error(&format!(
                    "[ACK] - Unable to forward packet to neither next hop nor SC. \n Packet: {}",
                    packet
                ));
                return;
            }

            self.log_debug(&format!(
                "[ACK] - Successfully sent flood response through SC. Packet: {}",
                packet
            ));
        }
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
        if let Err(msg) = self.send_packets_vec(packets, next_hop) {
            return Err(msg);
        }

        self.insert_packet_history(packets);

        Ok(())
    }
}
