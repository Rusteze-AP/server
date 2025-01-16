use super::Server;

use packet_forge::SessionIdT;

impl Server {
    /// Pop the corresponding fragment from `packet_history`
    pub(crate) fn ack_handler(&mut self, fragment_index: u64, session_id: SessionIdT) {
        let Some(entry) = self.packets_history.remove(&(fragment_index, session_id)) else {
            self.logger.log_error(&format!(
                "[SERVER-{}] Failed to remove [ ({}, {}) ] key from packet history",
                self.id, fragment_index, session_id
            ));
            return;
        };
        self.logger.log_info(&format!(
            "[SERVER-{}] Packet history updated, removed: {:?}",
            self.id, entry
        ));
    }
}
