pub mod linux;
pub mod windows;

use std::collections::HashSet;
use std::process::Output;

pub trait CommandExecutor: Send + Sync {
    fn execute(&self, cmd: &str, args: &[&str]) -> Result<Output, std::io::Error>;
}

pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn execute(&self, cmd: &str, args: &[&str]) -> Result<Output, std::io::Error> {
        std::process::Command::new(cmd).args(args).output()
    }
}

pub trait FirewallDriver: Send + Sync {
    /// Blocks the specified IP subnets under a rule associated with a PoP.
    fn block_pop(&self, pop: &str, ips: &[String]) -> Result<(), String>;

    /// Unblocks the specified PoP (deletes its corresponding rules).
    fn unblock_pop(&self, pop: &str, ips: &[String]) -> Result<(), String>;

    /// Queries the firewall to find which PoP codes are currently blocked.
    fn get_blocked_pops(&self) -> Result<HashSet<String>, String>;

    /// Clears all rules created by the application.
    fn clear_all(&self) -> Result<(), String>;
}

/// Factory function to return the correct driver for the active platform.
pub fn create_driver() -> Box<dyn FirewallDriver> {
    #[cfg(test)]
    {
        #[cfg(windows)]
        {
            Box::new(windows::WindowsFirewall::with_executor(Box::new(MockCommandExecutor::new())))
        }
        #[cfg(not(windows))]
        {
            Box::new(linux::LinuxFirewall::with_executor(Box::new(MockCommandExecutor::new())))
        }
    }
    #[cfg(not(test))]
    {
        #[cfg(windows)]
        {
            Box::new(windows::WindowsFirewall::new())
        }
        #[cfg(unix)]
        {
            Box::new(linux::LinuxFirewall::new())
        }
        #[cfg(not(any(windows, unix)))]
        {
            panic!("Unsupported platform");
        }
    }
}

#[cfg(test)]
pub struct MockCommandExecutor {
    pub commands: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    pub mock_stdout: std::sync::Mutex<String>,
    pub mock_success: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl MockCommandExecutor {
    pub fn new() -> Self {
        Self {
            commands: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            mock_stdout: std::sync::Mutex::new(String::new()),
            mock_success: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

#[cfg(test)]
fn make_exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
    #[cfg(not(any(unix, windows)))]
    {
        panic!("Unsupported platform status");
    }
}

#[cfg(test)]
impl CommandExecutor for MockCommandExecutor {
    fn execute(&self, cmd: &str, args: &[&str]) -> Result<Output, std::io::Error> {
        let cmd_str = format!("{} {}", cmd, args.join(" "));
        self.commands.lock().unwrap().push(cmd_str);
        
        let stdout = self.mock_stdout.lock().unwrap().clone().into_bytes();
        let code = if args.contains(&"-C") {
            1
        } else {
            if self.mock_success.load(std::sync::atomic::Ordering::SeqCst) { 0 } else { 1 }
        };
        
        Ok(Output {
            status: make_exit_status(code),
            stdout,
            stderr: Vec::new(),
        })
    }
}
