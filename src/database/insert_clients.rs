use packet_forge::ClientType;
use wg_internal::network::NodeId;

use super::Database;

impl Database {
    /// Insert the client into the tree. To use after the use of `contains_client`, this will replace any previous entry.
    pub(crate) fn insert_client(&self, id: NodeId, client_type: &ClientType) -> Result<(), String> {
        let serialized_type =
            bincode::serialize(&client_type).map_err(|e| format!("Serialization error: {e}"))?;
        let _ = self
            .clients_tree
            .insert(id.to_be_bytes(), serialized_type)
            .map_err(|e| format!("Error inserting song metadata: {e}"));
        Ok(())
    }

    pub(crate) fn remove_client(&self, id: NodeId) -> Result<Option<ClientType>, String> {
        match self.clients_tree.remove(id.to_be_bytes()) {
            Ok(Some(removed_value)) => {
                // Deserialize the removed value into ClientType
                bincode::deserialize(&removed_value)
                    .map(Some) // Wrap the ClientType in Option
                    .map_err(|e| format!("Deserialization error: {e}")) // Handle deserialization errors
            }
            Ok(None) => Ok(None), // No client found, return None
            Err(msg) => Err(format!(
                "An error occurred while removing the client: {msg}"
            )),
        }
    }
}
