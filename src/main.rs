mod privileges;
mod config;
mod sdr;
mod firewall;
mod ping;
mod tui;

fn main() {
    if !privileges::is_admin() {
        eprintln!("Error: This application requires administrative privileges to modify firewall rules.");
        #[cfg(unix)]
        eprintln!("Please run this application using: sudo ./valve-server-manager");
        #[cfg(windows)]
        eprintln!("Please run this application in an Administrator Command Prompt or PowerShell.");
        std::process::exit(1);
    }

    if let Err(e) = tui::run_tui() {
        eprintln!("Fatal Error: {}", e);
        std::process::exit(1);
    }
}

