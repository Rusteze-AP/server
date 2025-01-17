use crate::server::video_chunker::get_video_chunks;

use super::Server;

use packet_forge::{ChunkRequest, ChunkResponse};

impl Server {
    // TODO Implement handling Audio messages for server

    pub(crate) fn handle_req_video(&mut self, message: &ChunkRequest) {
        let video_chunks = get_video_chunks(message.file_hash);

        let Some(srh) = self.get_path(self.id, message.client_id) else {
            return;
        };
        let next_hop = srh.hops[srh.hop_index];

        for (index, chunk) in video_chunks.enumerate() {
            let Ok(chunk_index) = u32::try_from(index) else {
                self.logger.log_error(&format!(
                    "[SERVER-{}] Could not convert chunk_index to u32",
                    self.id
                ));
                return;
            };
            let chunk_res = ChunkResponse::new(message.file_hash, chunk_index, chunk.clone());

            // Disassemble ChunkResponse into Packets
            let packets = match self
                .packet_forge
                .disassemble(chunk_res.clone(), srh.clone())
            {
                Ok(packets) => packets,
                Err(msg) => {
                    self.logger.log_error(&format!("[SERVER-{}] Error disassembling ChunkResponse message! (log_info to see more information)", self.id));
                    self.logger.log_info(&format!(
                        "[SERVER-{}] {:?}\n Error: {}",
                        self.id, chunk_res, msg
                    ));
                    return;
                }
            };

            if let Err(msg) = self.send_save_packets(&packets, next_hop) {
                self.logger.log_error(&msg);
                return;
            }

            self.logger.log_info(&format!(
                "[SERVER-{}] Correctly forwarded: {:?}",
                self.id, chunk_res
            ));
        }
    }
}
