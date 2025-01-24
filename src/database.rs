mod getters;
mod insert_clients;
mod insert_songs;
mod insert_videos;

use serde::{Deserialize, Serialize};
use sled::{self, Tree};
use std::{collections::HashSet, fs, process};
use wg_internal::network::NodeId;

use packet_forge::{ClientType, FileHash, Metadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_type: ClientType,         // "songs" or "video"
    pub shared_files: HashSet<FileHash>, // Files shared by this client
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry<T> {
    pub file_metadata: T,
    pub peers: HashSet<NodeId>, // List of clients sharing the file
}

pub struct Database {
    db: sled::Db,
    video_tree: Tree,
    songs_tree: Tree,
    clients_tree: Tree,
    server_id: NodeId,
}

/// Build a key: prefix:id
pub(crate) fn construct_payload_key(prefix: &str, id: u16) -> Vec<u8> {
    let key = format!("{prefix}:{id}").as_bytes().to_vec();
    key
}

impl Database {
    /// Creates or opens a database at the specified path.
    pub fn new(database: &str, server_id: NodeId) -> Self {
        let db = sled::open(database).unwrap_or_else(|e| {
            eprintln!("Error opening database: {}", e);
            process::exit(1); // Exit the program with an error code
        });

        // Helper function to open trees and exit on error
        let open_tree = |tree_name: &str| {
            db.open_tree(tree_name).unwrap_or_else(|e| {
                eprintln!("Error opening {} tree: {}", tree_name, e);
                process::exit(1); // Exit the program with an error code
            })
        };

        let video_tree = open_tree("video");
        let songs_tree = open_tree("songs");
        let clients_tree = open_tree("clients");

        Database {
            db,
            video_tree,
            songs_tree,
            clients_tree,
            server_id,
        }
    }

    fn clear_database(&self) -> Result<(), String> {
        self.db
            .clear()
            .map_err(|e| format!("Error clearing database: {}", e))?;
        self.db
            .flush()
            .map_err(|e| format!("Error flushing database: {}", e))?;
        Ok(())
    }

    fn load_json_metadata<T: Metadata>(
        &self,
        json_file_path: &str,
        json_array: &str,
    ) -> Result<Vec<T>, String> {
        let file_content = fs::read_to_string(json_file_path)
            .map_err(|e| format!("Error reading file {}: {}", json_file_path, e))?;

        let json_data: serde_json::Value = serde_json::from_str(&file_content)
            .map_err(|e| format!("Error parsing JSON: {}", e))?;

        let songs_array = json_data[json_array]
            .as_array()
            .ok_or_else(|| format!("Invalid JSON: '{json_array}' is not an array"))?;

        songs_array
            .iter()
            .map(|song| {
                serde_json::from_value(song.clone())
                    .map_err(|e| format!("Invalid song data: {}", e))
            })
            .collect()
    }

    /// Initializes the database:
    /// - clears existing entries
    /// - checks for data from local files (songs and video).
    /// ### Arguments
    /// - `local_path`: the folder containing the two JSON files
    /// - `file_songs_name`: the name of the file with the song array. It must contain the extension (*.json)
    /// - `file_video_name`: the name of the file with the video array. It must contain the extension (*.json)
    pub fn init(
        &self,
        local_path: &str,
        file_songs_name: Option<&str>,
        file_video_name: Option<&str>,
    ) -> Result<(), String> {
        self.clear_database()?;

        if let Some(file_name) = file_songs_name {
            let songs_metadata_path = local_path.to_string() + "/" + file_name;
            let songs_array = self.load_json_metadata(&songs_metadata_path, "songs")?;
            self.insert_songs_from_vec(local_path, &songs_array)?;
        }

        if let Some(file_name) = file_video_name {
            let videos_metadata_path = local_path.to_string() + "/" + file_name;
            let videos_array = self.load_json_metadata(&videos_metadata_path, "videos")?;
            self.insert_videos_from_vec(local_path, &videos_array)?;
        }

        Ok(())
    }
}
