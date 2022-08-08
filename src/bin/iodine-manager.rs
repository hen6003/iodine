use bincode::config;
use std::os::unix::net::UnixStream;

fn main() {
    let command = iodine::SockMessage {
        service: "test".to_string(),
        command: iodine::ServiceCommands::Up,
    };

    let mut stream = UnixStream::connect(iodine::SOCK_LOCATION).unwrap();

    bincode::encode_into_std_write(command, &mut stream, config::standard()).unwrap();
}
