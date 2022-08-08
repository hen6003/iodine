use bincode::config;
use crossbeam::channel;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command};
use std::thread;
use std::{default, fs, path::PathBuf};

struct Service {
    data: iodine::ServiceFile,
}

impl Service {
    fn start(self) -> ServiceThread {
        let (send, commands) = channel::unbounded();
        let (main_thread, recv) = channel::unbounded();

        thread::spawn(move || {
            let mut want_up = true;

            loop {
                if want_up {
                    // Start service
                    let mut process = self
                        .data
                        .commands
                        .get("start")
                        .expect("Start command required")
                        .spawn()
                        .unwrap();

                    main_thread.send(process.id()).unwrap();

                    // If oneshot, service is no longer wanted up
                    want_up = !self.data.service.oneshot;

                    // Wait for process to exit
                    process.wait().unwrap();

                    // Check for message from main thread
                    if let Ok(command) = commands.try_recv() {
                        use iodine::ServiceCommands;

                        match command {
                            ServiceCommands::Down | ServiceCommands::Kill => want_up = false,
                            ServiceCommands::Up | ServiceCommands::Restart => want_up = true,
                            ServiceCommands::Status => todo!(),
                        }
                    }
                } else {
                    // Check for message from main thread
                    if let Ok(command) = commands.recv() {
                        use iodine::ServiceCommands;

                        match command {
                            ServiceCommands::Down | ServiceCommands::Kill => want_up = false,
                            ServiceCommands::Up | ServiceCommands::Restart => want_up = true,
                            ServiceCommands::Status => todo!(),
                        }
                    }
                }
            }
        });

        ServiceThread { recv, send }
    }
}

impl From<iodine::ServiceFile> for Service {
    fn from(data: iodine::ServiceFile) -> Self {
        Self { data }
    }
}

struct ServiceThread {
    recv: channel::Receiver<u32>,
    send: channel::Sender<iodine::ServiceCommands>,
}

struct ServiceManager {
    services_dir: PathBuf,
    running_services: HashMap<String, ServiceThread>,
    services: HashMap<String, Service>,
    services_provides: HashMap<String, Vec<String>>,
}

impl ServiceManager {
    fn scan_service_dir(&mut self) {
        let service_paths = fs::read_dir(self.services_dir.as_path()).unwrap();

        // Setup service names for starting later
        for service_path in service_paths {
            let service_path = service_path.unwrap().path();

            println!("path: {:?}", service_path);

            let mut file_data = String::new();

            fs::File::open(&service_path)
                .unwrap()
                .read_to_string(&mut file_data)
                .unwrap();

            let service: iodine::ServiceFile = toml::from_str(&file_data).unwrap();

            // Get name
            let mut name = service_path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            if let Some(n) = &service.service.name {
                name = n.to_string()
            }

            let provides = service.service.provides.clone();

            self.services
                .insert(name.to_string(), Service::from(service));

            if let Some(provides) = provides {
                if let Some(names) = self.services_provides.get_mut(&provides) {
                    names.push(name.to_string());
                } else {
                    self.services_provides
                        .insert(provides, vec![name.to_string()]);
                }
            }
        }
    }

    // Start services
    fn start(&mut self) {
        for service in self.services.drain() {
            let (name, service) = service;

            self.running_services.insert(name, service.start());
        }
    }

    fn handle_client(&mut self, mut stream: UnixStream) {
        // Read and decode message
        let message: iodine::SockMessage =
            bincode::decode_from_std_read(&mut stream, config::standard())
                .expect("Failed to decode message");

        // Send message
        let service = self.running_services.get(&message.service).unwrap();
        service.send.send(message.command).unwrap();

        // Make sure program is killed
        let pid = service.recv.recv().unwrap();
        if pid != 0 {
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::SIGTERM,
            )
            .unwrap();
        }
    }

    fn init(&mut self) {
        // Read all services
        self.scan_service_dir();

        // Start all services
        self.start();

        // Start listening to sock
        let listener = UnixListener::bind(iodine::SOCK_LOCATION).unwrap();

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    self.handle_client(stream);
                }
                Err(err) => {
                    panic!("{}", err)
                }
            }
        }
    }
}

impl default::Default for ServiceManager {
    fn default() -> Self {
        Self {
            running_services: HashMap::new(),
            services_dir: PathBuf::from("services/"),
            services: HashMap::new(),
            services_provides: HashMap::new(),
        }
    }
}

fn main() {
    ServiceManager::default().init();
}
