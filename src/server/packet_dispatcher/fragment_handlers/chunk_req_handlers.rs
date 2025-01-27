use crate::server::video_chunker::get_video_chunks;

use super::Server;

use packet_forge::{ChunkRequest, ChunkResponse, ClientType};

impl Server {
    pub(crate) fn handle_chunk_request(&mut self, message: &ChunkRequest) {
        let client_type = self.database.get_client_type(message.client_id);

        let res = match client_type {
            Ok(ClientType::Song) => self.handle_song_req(message),
            Ok(ClientType::Video) => self.handle_video_req(message),
            Err(msg) => Err(msg),
        };

        if let Err(msg) = res {
            self.logger.log_error(&msg);
        }
    }

    // TODO Implement handling Audio messages for server
    fn handle_song_req(&mut self, message: &ChunkRequest) -> Result<(), String> {
        todo!()
    }

    /// Get the requested video data from the database and sends its chunk to the client
    fn handle_video_req(&mut self, message: &ChunkRequest) -> Result<(), String> {
        // Retrieve data of the video from the database
        let video_data = self.database.get_video_payload(message.file_hash)?;

        // Split the video into chunks
        let video_chunks = get_video_chunks(video_data);

        let Some(srh) = self.get_path(self.id, message.client_id) else {
            return Err("An error occurred: failed to get path".to_string());
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
                        "{:?}\n Error while disassembling: {}",
                        chunk_res, msg
                    ));
                }
            };

            self.send_save_packets(&packets, next_hop)?;

            self.logger
                .log_info(&format!("Correctly forwarded: {:?}", chunk_res));
        }
        Ok(())
    }
}
