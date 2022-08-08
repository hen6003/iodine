use bincode::{Decode, Encode};
use nix::unistd::{Group, User};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process;

pub const SOCK_LOCATION: &str = "iodine.sock";

#[derive(Serialize, Deserialize, Debug, Clone, Decode, Encode)]
pub struct SockMessage {
    pub service: String,
    pub command: ServiceCommands,
}

#[derive(Serialize, Deserialize, Debug, Clone, Decode, Encode)]
pub enum ServiceCommands {
    Down, // Sends term
    Kill, // Sends kill
    Up,
    Restart, // Sends term
    Status,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceFile {
    pub info: Option<Info>,

    #[serde(default)]
    pub service: Service,

    pub commands: HashMap<String, Command>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Info {
    pub description: String,
    pub homepage: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Service {
    pub name: Option<String>,
    pub provides: Option<String>,

    #[serde(default)]
    pub depends: Vec<String>,

    #[serde(default)]
    pub oneshot: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Command {
    command: String,
    user: Option<String>,
    group: Option<String>,
    directory: Option<String>,
}

impl Command {
    pub fn spawn(&self) -> std::io::Result<process::Child> {
        let mut command = process::Command::new("sh");
        command.arg("-c").arg(&self.command);

        if let Some(user) = &self.user {
            let user = User::from_name(&user)?.ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "User not found",
            ))?;

            command.uid(user.uid.as_raw());
            command.gid(user.gid.as_raw());
        }

        if let Some(group) = &self.group {
            let group = Group::from_name(&group)?.ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Group not found",
            ))?;

            command.gid(group.gid.as_raw());
        }

        if let Some(directory) = &self.directory {
            command.current_dir(directory);
        }

        command.spawn()
    }
}
