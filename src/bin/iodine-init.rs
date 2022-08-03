use nix::sys::signal::{self, SigSet};
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use std::env;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const SIGNALS: [i32; 5] = [SIGUSR1, SIGUSR2, SIGCHLD, SIGALRM, SIGINT];
const TIME: u32 = 30;

// Wrapper for easy calling with correct magic values
unsafe fn sys_reboot(cmd: u32) -> Result<(), nc::Errno> {
    nc::reboot(nc::LINUX_REBOOT_MAGIC1, nc::LINUX_REBOOT_MAGIC2, cmd, 0)
}

#[derive(Clone, Copy)]
enum ShutdownType {
    PowerOff,
    Reboot,
    CtrlAltDelete,
}

enum Stage {
    Start,
    System,
    Shutdown(ShutdownType),
}

fn spawn_stage(stage: Stage) -> std::process::Child {
    let file = match stage {
        Stage::Start => "/etc/iodine/start",
        Stage::System => "/etc/iodine/service",
        Stage::Shutdown(_) => "/etc/iodine/shutdown",
    };

    let mut command = std::process::Command::new(file);

    if let Stage::Shutdown(t) = stage {
        command.arg(match t {
            ShutdownType::PowerOff => "poweroff",
            ShutdownType::Reboot => "reboot",
            ShutdownType::CtrlAltDelete => "ctrlaltdelete",
        });
    }

    command.spawn().unwrap()
}

fn startup() -> bool {
    let mut child = spawn_stage(Stage::Start);

    // Block all signals
    let sigset = SigSet::all();
    signal::sigprocmask(signal::SigmaskHow::SIG_BLOCK, Some(&sigset), None).unwrap();

    // Check return code of startup
    let ret = match child.wait().unwrap().code() {
        Some(code) => code == 111,
        None => false,
    };

    // Unblock all signals
    signal::sigprocmask(signal::SigmaskHow::SIG_UNBLOCK, Some(&sigset), None).unwrap();

    ret
}

fn shutdown(t: ShutdownType) {
    spawn_stage(Stage::Shutdown(t));

    let reboot_cmd = match t {
        ShutdownType::PowerOff => nc::LINUX_REBOOT_CMD_POWER_OFF,
        ShutdownType::Reboot | ShutdownType::CtrlAltDelete => nc::LINUX_REBOOT_CMD_RESTART,
    };

    unsafe {
        nc::sync().unwrap();
        sys_reboot(reboot_cmd).unwrap();
    }
}

fn init() -> ! {
    if std::process::id() != 1 {
        panic!("Unable to start init - must be PID 1");
    }

    // Spawn first init script
    if !startup() {
        // Startup failed - shutdown
        shutdown(ShutdownType::PowerOff);
    }

    // Install handler
    let mut signals = Signals::new(&SIGNALS).unwrap();

    // Disable kernel Ctrl-Alt-Delete and send SIGINT's to this process
    unsafe { sys_reboot(nc::LINUX_REBOOT_CMD_CAD_OFF) }.unwrap();

    // Spawn services
    spawn_stage(Stage::System);

    // Wait for signals
    loop {
        for signal in signals.pending() {
            match signal {
                // Spawn shutdown stage
                SIGUSR1 => shutdown(ShutdownType::PowerOff),
                SIGUSR2 => shutdown(ShutdownType::Reboot),
                SIGINT => shutdown(ShutdownType::CtrlAltDelete),

                // Handle children
                SIGALRM | SIGCHLD => {
                    nix::sys::wait::waitpid(
                        nix::unistd::Pid::from_raw(-1),
                        Some(nix::sys::wait::WaitPidFlag::WNOHANG),
                    )
                    .unwrap();

                    unsafe {
                        _ = nc::alarm(TIME);
                    }
                }

                _ => unreachable!(),
            }
        }
    }
}

fn main() {
    let mut args = env::args();

    match args.nth(1) {
        Some(s) => match s.as_str() {
            "-v" => println!("Iodine version: {}", VERSION),
            _ => println!("Usage: iodine-init [-v]"),
        },
        None => init(),
    }
}
