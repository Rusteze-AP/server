use super::Server;

use packet_forge::*;
use wg_internal::network::NodeId;

impl Server {
    fn add_new_song(&self, song_metadata: &SongMetaData, client_id: NodeId) {
        if let Err(msg) = self.database.insert_song_peer(song_metadata, client_id) {
            self.logger.log_error(&msg);
        }
        self.logger
            .log_debug(&format!("Added new Song [ {song_metadata:?} ]"));
    }

    fn remove_existing_song(&self, song_id: FileHash) {
        if let Err(msg) = self.database.remove_song(song_id) {
            self.logger.log_error(&msg);
        }
        self.logger
            .log_debug(&format!("Removed Song with ID [ {song_id} ]"));
    }

    fn add_new_video(&self, video_metadata: &VideoMetaData, client_id: NodeId) {
        if let Err(msg) = self.database.insert_video_peer(video_metadata, client_id) {
            self.logger.log_error(&msg);
        }
        self.logger
            .log_debug(&format!("Added new Video [ {video_metadata:?} ]"));
    }

    fn remove_existing_video(&self, video_id: FileHash) {
        if let Err(msg) = self.database.remove_video(video_id) {
            self.logger.log_error(&msg);
        }
        self.logger
            .log_debug(&format!("Removed Video with ID [ {video_id} ]"));
    }

    /// Add client information to the database
    /// - if client is already subscribed exit
    /// - Add client to `client_tree` (id -> type):
    ///     - if audio adds info to `audio_tree`
    ///     - if video adds info to `video_tree`
    pub(crate) fn subscribe_client(&mut self, message: &SubscribeClient) {
        // Check if client is already subscribed
        if self.database.contains_client(message.client_id) {
            self.logger.log_warn(&format!(
                "Received SubscribeClient message but [CLIENT {}] already exists!",
                message.client_id
            ));
            return;
        }

        // Add client to database `client_tree`
        if let Err(msg) = self
            .database
            .insert_client(message.client_id, message.client_type.clone())
        {
            self.logger.log_error(&msg);
            return;
        }

        // Add files to song or video
        for file in &message.available_files {
            match file {
                FileMetadata::Song(song_metadata) => {
                    if let Err(msg) = Self::check_hash(song_metadata.id, song_metadata) {
                        self.logger.log_error(&msg);
                        continue;
                    }
                    self.add_new_song(song_metadata, message.client_id);
                }
                FileMetadata::Video(video_metadata) => {
                    if let Err(msg) = Self::check_hash(video_metadata.id, video_metadata) {
                        self.logger.log_error(&msg);
                        continue;
                    }
                    self.add_new_video(video_metadata, message.client_id);
                }
            };
        }

        self.logger.log_info(&format!(
            "Client {} subscribed with success!",
            message.client_id
        ));
    }

    /// Given a client, remove or add information about a file that it shares.
    pub(crate) fn update_file_list(&mut self, message: &UpdateFileList) {
        if !self.database.contains_client(message.client_id) {
            self.logger.log_warn(&format!(
                "Received UpdateFileList for [CLIENT-{}] but no client was found. File list: {:?}",
                message.client_id, message.updated_files
            ));
            return;
        }

        for (file_metadata, file_status) in &message.updated_files {
            match file_metadata {
                FileMetadata::Song(song_metadata) => {
                    if let Err(msg) = Self::check_hash(song_metadata.id, song_metadata) {
                        self.logger.log_error(&msg);
                    }

                    match file_status {
                        FileStatus::New => self.add_new_song(song_metadata, message.client_id),
                        FileStatus::Deleted => self.remove_existing_song(song_metadata.id),
                    }
                }
                FileMetadata::Video(video_metadata) => {
                    if let Err(msg) = Self::check_hash(video_metadata.id, video_metadata) {
                        self.logger.log_error(&msg);
                    }

                    match file_status {
                        FileStatus::New => self.add_new_video(video_metadata, message.client_id),
                        FileStatus::Deleted => self.remove_existing_video(video_metadata.id),
                    }
                }
            }
        }
        self.logger
            .log_info(&format!("Client-{} file list updated!", message.client_id));
    }

    /// Send all the file available to the requesting client
    pub(crate) fn send_file_list(&mut self, message: &RequestFileList) {
        let client_type = self.database.get_client_type(message.client_id);

        let response_file_list = match client_type {
            Ok(ClientType::Song) => {
                let songs = self.database.get_all_songs_metadata();
                if let Err(msg) = songs {
                    self.logger.log_error(&msg);
                    return;
                }
                ResponseFileList::new(
                    songs
                        .unwrap()
                        .iter()
                        .map(|song| FileMetadata::Song(song.clone()))
                        .collect(),
                )
            }
            Ok(ClientType::Video) => {
                let videos = self.database.get_all_videos_metadata();
                if let Err(msg) = videos {
                    self.logger.log_error(&msg);
                    return;
                }
                ResponseFileList::new(
                    videos
                        .unwrap()
                        .iter()
                        .map(|video| FileMetadata::Video(video.clone()))
                        .collect(),
                )
            }
            Err(msg) => {
                self.logger.log_error(&msg);
                return;
            }
        };

        // Retrieve best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, message.client_id) else {
            self.logger
                .log_error("An error occurred: failed to get routing path");
            return;
        };

        // Disassemble ResponseFileList into Packets
        let packets = match self
            .packet_forge
            .disassemble(response_file_list.clone(), &srh)
        {
            Ok(packets) => packets,
            Err(msg) => {
                self.logger.log_error("Error disassembling ResponseFileList message! (log_debug to see more information)");
                self.logger
                    .log_debug(&format!("[VERBOSE] {response_file_list:?}\n Error: {msg}",));
                return;
            }
        };

        let next_hop = srh.hops[srh.hop_index];
        if let Err(msg) = self.send_save_packets(&packets, next_hop) {
            self.logger.log_error(&msg);
            return;
        }

        self.logger.log_info("ResponseFileList sent successfully!");
    }

    /// Send a list of peers from which the requested file can be downloaded
    pub(crate) fn send_peer_list(&mut self, message: &RequestPeerList) {
        let client_type = self.database.get_client_type(message.client_id);

        let file_peers = match client_type {
            Ok(ClientType::Song) => {
                // Retrieve the requested file
                let song = self.database.get_song_entry(message.file_hash);
                if let Err(msg) = song {
                    self.logger.log_error(&msg);
                    return;
                }

                song.unwrap().peers
            }
            Ok(ClientType::Video) => {
                // Retrieve the requested file
                let song = self.database.get_video_entry(message.file_hash);
                if let Err(msg) = song {
                    self.logger.log_error(&msg);
                    return;
                }

                song.unwrap().peers
            }
            Err(msg) => {
                self.logger.log_error(&msg);
                return;
            }
        };

        // Create the vector to send to the client
        let peers_info: Vec<PeerInfo> = file_peers
            .iter()
            .filter_map(|peer| {
                if let Some(srh) = self.get_path(*peer, message.client_id) {
                    return Some(PeerInfo {
                        client_id: *peer,
                        path: srh.hops,
                    });
                }
                None
            })
            .collect();

        // Create response
        let file_list = ResponsePeerList::new(message.file_hash, peers_info);

        // Retrieve best path from server to client otherwise return
        let Some(srh) = self.get_path(self.id, message.client_id) else {
            self.logger
                .log_error("An error occurred: failed to get routing path");
            return;
        };

        // Disassemble ResponsePeerList into Packets
        let packets = match self.packet_forge.disassemble(file_list.clone(), &srh) {
            Ok(packets) => packets,
            Err(msg) => {
                self.logger.log_error("Error disassembling ResponsePeerList message! (log_info to see more information)");
                self.logger
                    .log_info(&format!("[VERBOSE] {file_list:?}\n Error: {msg}"));
                return;
            }
        };

        let next_hop = srh.hops[srh.hop_index];
        if let Err(msg) = self.send_save_packets(&packets, next_hop) {
            self.logger.log_error(&msg);
            return;
        }

        self.logger.log_info("ResponsePeerList sent successfully!");
    }

    /// Unsubscribe the information of a client
    pub(crate) fn unsubscribe_client(&mut self, message: &UnsubscribeClient) {
        // Check if client is subscribed
        if !self.database.contains_client(message.client_id) {
            self.logger.log_warn(&format!(
                "Received UnsubscribeClient for [CLIENT-{}] but no client was found.",
                message.client_id
            ));
            return;
        }

        // Remove Client from clients HashMap
        let Ok(client_type) = self.database.remove_client(message.client_id) else {
            self.logger.log_error(&format!(
                "No [CLIENT {}] found: could not remove it from clients!",
                message.client_id
            ));
            return;
        };

        let res = match client_type {
            Some(ClientType::Song) => self.database.remove_peer_from_songs(message.client_id),
            Some(ClientType::Video) => self.database.remove_peer_from_videos(message.client_id),
            None => Err(format!(
                "Remove client returned with None. No [CLIENT-{}] removed.",
                message.client_id
            )),
        };

        if let Err(msg) = res {
            self.logger.log_error(&msg);
        }

        self.logger.log_info(&format!(
            "Client-{} unsubscribed with success!",
            message.client_id
        ));
    }
}
