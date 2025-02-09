use super::Server;

use crossbeam::channel::Sender;
use wg_internal::{controller::DroneCommand, network::NodeId, packet::Packet};

impl Server {
    pub(crate) fn remove_sender(&mut self, id: NodeId) -> Result<(), String> {
        let res = self.packet_send.remove(&id);
        if res.is_none() {
            return Err(format!("[REMOVE SENDER] - Sender with id {id} not found"));
        }
        self.logger
            .log_info(&format!("[REMOVE SENDER] - Sender with id {id} removed"));
        Ok(())
    }

    pub(crate) fn add_sender(&mut self, id: NodeId, sender: &Sender<Packet>) -> Result<(), String> {
        let res = self.packet_send.insert(id, sender.clone());
        if res.is_some() {
            return Err(format!("[ADD SENDER] - Sender with id {id} already exists",));
        }
        self.logger
            .log_info(&format!("[ADD SENDER] - Sender with id {id} added"));
        Ok(())
    }

    pub(crate) fn command_dispatcher(&mut self, command: &DroneCommand) {
        if !self.terminated {
            let res = match command {
                DroneCommand::RemoveSender(id) => {
                    self.init_flood_request();
                    self.remove_sender(*id)
                }
                DroneCommand::AddSender(id, sender) => {
                    self.init_flood_request();
                    self.add_sender(*id, sender)
                }
                DroneCommand::Crash => {
                    self.logger
                        .log_info("[SC COMMAND] - Received crash command. Terminating!");
                    self.terminated = true;
                    Ok(())
                }
                DroneCommand::SetPacketDropRate(_) => {
                    Err("[SC COMMAND] - Received unhandled SC command (ChangePdr)!".to_string())
                }
            };

            if let Err(err) = res {
                self.logger.log_error(&err);
            }
        }
    }
}
