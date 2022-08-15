use bincode::config;
use std::os::unix::net::UnixStream;

fn command_from_args() -> iodine::SockMessage {
    let mut args = std::env::args();

    let command = args.nth(1).expect("Provide a command");
    let service = args.next().expect("Provide a service");

    let command = match command.as_str() {
        "down" => iodine::ServiceCommands::Down, // Sends term
        "kill" => iodine::ServiceCommands::Kill, // Sends kill
        "up" => iodine::ServiceCommands::Up,
        "restart" => iodine::ServiceCommands::Restart, // Sends term
        "status" => iodine::ServiceCommands::Status,

        _ => panic!("Unknown command"),
    };

    iodine::SockMessage { service, command }
}

fn main() {
    let message = command_from_args();
    let mut stream = UnixStream::connect(iodine::SOCK_LOCATION).unwrap();

    bincode::encode_into_std_write(&message, &mut stream, config::standard()).unwrap();

    let data: iodine::ServiceStatus =
        bincode::decode_from_std_read(&mut stream, config::standard()).unwrap();

    println!("{:?}", data);
}
