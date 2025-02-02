use crate::server::video_chunker::get_video_chunks;

use super::Server;

use bytes::Bytes;
use packet_forge::{ChunkRequest, ChunkResponse, ClientType, Index};
use wg_internal::network::SourceRoutingHeader;

impl Server {
    pub(crate) fn handle_chunk_request(
        &mut self,
        message: &ChunkRequest,
        addressee_srh: &SourceRoutingHeader,
    ) {
        let client_type = self.database.get_client_type(message.client_id);

        let res = match client_type {
            Ok(ClientType::Song) => self.handle_song_req(message, addressee_srh),
            Ok(ClientType::Video) => self.handle_video_req(message, addressee_srh),
            Err(msg) => Err(msg),
        };

        if let Err(msg) = res {
            self.logger.log_error(&msg);
        }
    }

    /// Get the requested song data from the database and sends its chunk to the client
    fn handle_song_req(
        &mut self,
        message: &ChunkRequest,
        addressee_srh: &SourceRoutingHeader,
    ) -> Result<(), String> {
        // For each index in ChunkRequest send ChunkResponse
        if let Index::Indexes(vec) = &message.chunk_index {
            for chunk_index in vec {
                // Get segment from db
                let prefix = &format!("ts{chunk_index}");
                let segment = self.database.get_song_payload(prefix, message.file_hash)?;

                // Build ChunkResponse
                let chunk_data = Bytes::from(segment);
                let chunk_res = ChunkResponse::new(message.file_hash, *chunk_index, chunk_data);

                // Retrieve new best path from server to client, otherwise use incoming one
                let srh = match self.get_path(self.id, message.client_id) {
                    Some(new_srh) => new_srh,
                    None => {
                        self.logger
                            .log_error("[CHUNK RESPONSE - SONG] An error occurred: failed to get routing path, using reversed sender path");
                        addressee_srh.clone()
                    }
                };
                let next_hop = srh.hops[srh.hop_index];

                // Disassemble ChunkResponse into Packets
                let packets = match self.packet_forge.disassemble(chunk_res.clone(), &srh) {
                    Ok(packets) => packets,
                    Err(msg) => {
                        return Err(format!(
                            "{chunk_res:?}\n Error while disassembling song: {msg}"
                        ));
                    }
                };

                self.send_save_packets(&packets, next_hop)?;

                self.logger.log_info(&format!(
                    "[CHUNK RESPONSE - SONG] Forwarded chunk for song: {} to client-{}",
                    message.file_hash, message.client_id
                ));
            }
            return Ok(());
        }

        Err("ChunkRequest for songs does not handle requests for all chunks!".to_string())
    }

    /// Get the requested video data from the database and sends its chunk to the client
    fn handle_video_req(
        &mut self,
        message: &ChunkRequest,
        addressee_srh: &SourceRoutingHeader,
    ) -> Result<(), String> {
        // Retrieve data of the video from the database
        let video_data = self.database.get_video_payload(message.file_hash)?;

        // Split the video into chunks
        let video_chunks = get_video_chunks(video_data);

        // Retrieve new best path from server to client, otherwise use incoming one
        let srh = match self.get_path(self.id, message.client_id) {
            Some(new_srh) => new_srh,
            None => {
                self.logger
                            .log_error("[CHUNK RESPONSE - VIDEO] An error occurred: failed to get routing path, using reversed sender path");
                addressee_srh.clone()
            }
        };
        let next_hop = srh.hops[srh.hop_index];

        for (index, chunk) in video_chunks.enumerate() {
            let Ok(chunk_index) = u32::try_from(index) else {
                return Err("Could not convert chunk_index to u32".to_string());
            };
            let chunk_res = ChunkResponse::new(message.file_hash, chunk_index, chunk.clone());

            // Disassemble ChunkResponse into Packets
            let packets = match self.packet_forge.disassemble(chunk_res.clone(), &srh) {
                Ok(packets) => packets,
                Err(msg) => {
                    return Err(format!(
                        "{chunk_res:?}\n Error while disassembling video: {msg}"
                    ));
                }
            };

            self.send_save_packets(&packets, next_hop)?;

            self.logger.log_info(&format!(
                "[CHUNK RESPONSE - VIDEO] Forwarded chunk for video {} to client-{}",
                message.file_hash, message.client_id
            ));
        }
        Ok(())
    }
}
