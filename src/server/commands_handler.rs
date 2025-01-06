use super::Server;

use wg_internal::controller::DroneCommand;

impl Server {
    pub(crate) fn command_dispatcher(&mut self, command: &DroneCommand) {
        match command {
            DroneCommand::Crash => {
                self.logger
                    .log_debug(&format!("Server {} received a crash command", self.id));
                self.terminated = true;
            }
            _ => {
                println!("Server {} unimplemented command", self.id);
            }
        }
    }
}
