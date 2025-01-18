mod getters;
mod inserts;

use serde::{Deserialize, Serialize};
use sled::{self, Tree};
use std::{collections::HashSet, fs, process};
use wg_internal::network::NodeId;

use packet_forge::{ClientType, FileHash, SongMetadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_type: ClientType,         // "audio" or "video"
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
}

/// Build a key: prefix:id
pub(crate) fn construct_payload_key(prefix: &str, id: u16) -> Vec<u8> {
    let key = format!("{prefix}:{id}").as_bytes().to_vec();
    key
}

impl Database {
    /// Creates or opens a database at the specified path.
    pub fn new(database: &str) -> Self {
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
        let songs_tree = open_tree("audio");
        let clients_tree = open_tree("clients");

        Database {
            db,
            video_tree,
            songs_tree,
            clients_tree,
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

    /// Find a specific file by name in the given directory
    fn find_specific_file(&self, local_path: &str, file_name: &str) -> Result<String, String> {
        let dir_entries = fs::read_dir(local_path)
            .map_err(|e| format!("Error reading directory {}: {}", local_path, e))?;

        for entry in dir_entries {
            let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
                return Ok(path.to_string_lossy().into_owned());
            }
        }

        Err(format!(
            "File {} not found in directory {}",
            file_name, local_path
        ))
    }

    fn load_json_metadata(&self, json_file_path: &str) -> Result<Vec<SongMetadata>, String> {
        let file_content = fs::read_to_string(json_file_path)
            .map_err(|e| format!("Error reading file {}: {}", json_file_path, e))?;

        let json_data: serde_json::Value = serde_json::from_str(&file_content)
            .map_err(|e| format!("Error parsing JSON: {}", e))?;

        let songs_array = json_data["audio"]
            .as_array()
            .ok_or_else(|| "Invalid JSON: 'audio' is not an array".to_string())?;

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
    /// - checks for data from local files (audio and video).
    /// ### Arguments
    /// - `local_path`: the folder containing the two JSON files
    /// - `audio_name`: the name of the file with the audio array (even empty). It must contain the extension (*.json)
    /// - `video_name`: the name of the file with the video array (even empty). It must contain the extension (*.json)
    pub fn init(&self, local_path: &str, audio_name: &str, video_name: &str) -> Result<(), String> {
        self.clear_database()?;

        let songs_metadata_path = self.find_specific_file(local_path, audio_name)?;
        // let videos_metadata_path = self.find_specific_file(local_path, video_name)?;

        let songs_array = self.load_json_metadata(&songs_metadata_path)?;
        // let videos_array = self.load_json_metadata(&videos_metadata_path)?;

        self.insert_songs_from_vec(Some(local_path), &songs_array)?;
        // self.insert_videos_from_vec(local_path, &videos_array)?;

        Ok(())
    }
}
