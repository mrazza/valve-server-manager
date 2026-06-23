use std::collections::HashSet;
use super::{FirewallDriver, CommandExecutor, SystemCommandExecutor};

#[allow(dead_code)]
pub struct LinuxFirewall {
    executor: Box<dyn CommandExecutor>,
}

#[allow(dead_code)]
impl LinuxFirewall {
    pub fn new() -> Self {
        Self::with_executor(Box::new(SystemCommandExecutor))
    }

    pub fn with_executor(executor: Box<dyn CommandExecutor>) -> Self {
        let firewall = Self { executor };
        let _ = firewall.init_chain();
        firewall
    }

    fn init_chain(&self) -> Result<(), String> {
        // Create VSM_BLOCKS chain (ignores error if it already exists)
        let _ = self.executor.execute("iptables", &["-N", "VSM_BLOCKS"]);

        // Check if jump from OUTPUT exists: iptables -C OUTPUT -j VSM_BLOCKS
        let jump_exists = self.executor.execute("iptables", &["-C", "OUTPUT", "-j", "VSM_BLOCKS"])
            .map(|out| out.status.success())
            .unwrap_or(false);

        if !jump_exists {
            let status = self.executor.execute("iptables", &["-I", "OUTPUT", "1", "-j", "VSM_BLOCKS"])
                .map_err(|e| format!("Failed to insert VSM_BLOCKS jump: {}", e))?;
            if !status.status.success() {
                return Err("Failed to insert jump in OUTPUT chain".to_string());
            }
        }

        Ok(())
    }

    fn has_rule(&self, subnet: &str, pop: &str) -> bool {
        let comment = format!("VSM_Block_{}", pop);
        self.executor.execute("iptables", &[
            "-C", "VSM_BLOCKS",
            "-d", subnet,
            "-j", "DROP",
            "-m", "comment",
            "--comment", &comment,
        ])
        .map(|out| out.status.success())
        .unwrap_or(false)
    }
}

impl FirewallDriver for LinuxFirewall {
    fn block_pop(&self, pop: &str, ips: &[String]) -> Result<(), String> {
        let _ = self.init_chain();

        let comment = format!("VSM_Block_{}", pop);
        for ip in ips {
            if !self.has_rule(ip, pop) {
                let status = self.executor.execute("iptables", &[
                    "-A", "VSM_BLOCKS",
                    "-d", ip,
                    "-j", "DROP",
                    "-m", "comment",
                    "--comment", &comment,
                ])
                .map_err(|e| format!("Failed to execute iptables: {}", e))?;
                
                if !status.status.success() {
                    return Err(format!("iptables block failed for {}", ip));
                }
            }
        }
        Ok(())
    }

    fn unblock_pop(&self, pop: &str, ips: &[String]) -> Result<(), String> {
        let comment = format!("VSM_Block_{}", pop);
        for ip in ips {
            while self.has_rule(ip, pop) {
                let _ = self.executor.execute("iptables", &[
                    "-D", "VSM_BLOCKS",
                    "-d", ip,
                    "-j", "DROP",
                    "-m", "comment",
                    "--comment", &comment,
                ]);
            }
        }
        Ok(())
    }

    fn get_blocked_pops(&self) -> Result<HashSet<String>, String> {
        let output = self.executor.execute("iptables", &["-S", "VSM_BLOCKS"])
            .map_err(|e| format!("Failed to read iptables rules: {}", e))?;

        if !output.status.success() {
            return Ok(HashSet::new());
        }

        let rules = String::from_utf8_lossy(&output.stdout);
        let mut blocked = HashSet::new();

        for line in rules.lines() {
            if line.contains("comment \"VSM_Block_") {
                if let Some(pos) = line.find("VSM_Block_") {
                    let start = pos + "VSM_Block_".len();
                    let pop_code_part = &line[start..];
                    let pop_code = pop_code_part.trim_end_matches('"').trim();
                    if !pop_code.is_empty() {
                        blocked.insert(pop_code.to_string());
                    }
                }
            }
        }

        Ok(blocked)
    }

    fn clear_all(&self) -> Result<(), String> {
        let _ = self.executor.execute("iptables", &["-F", "VSM_BLOCKS"]);
        let _ = self.executor.execute("iptables", &["-D", "OUTPUT", "-j", "VSM_BLOCKS"]);
        let _ = self.executor.execute("iptables", &["-X", "VSM_BLOCKS"]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firewall::MockCommandExecutor;

    #[test]
    fn test_linux_block_pop() {
        let mock_exec = Box::new(MockCommandExecutor::new());
        let cmd_list = mock_exec.commands.clone();
        
        let firewall = LinuxFirewall::with_executor(mock_exec);
        
        let res = firewall.block_pop("sea", &["192.69.96.0/22".to_string()]);
        assert!(res.is_ok());
        
        let cmds = cmd_list.lock().unwrap();
        assert!(cmds.iter().any(|c| c.contains("-N VSM_BLOCKS")));
        assert!(cmds.iter().any(|c| c.contains("-A VSM_BLOCKS -d 192.69.96.0/22 -j DROP -m comment --comment VSM_Block_sea")));
    }

    #[test]
    fn test_linux_get_blocked_pops() {
        let mock_exec = Box::new(MockCommandExecutor::new());
        *mock_exec.mock_stdout.lock().unwrap() = "-N VSM_BLOCKS\n-A VSM_BLOCKS -d 192.69.96.0/22 -j DROP -m comment --comment \"VSM_Block_sea\"\n-A VSM_BLOCKS -d 155.133.226.0/24 -j DROP -m comment --comment \"VSM_Block_fra\"\n".to_string();
        
        let firewall = LinuxFirewall::with_executor(mock_exec);
        let blocked = firewall.get_blocked_pops().unwrap();
        
        assert_eq!(blocked.len(), 2);
        assert!(blocked.contains("sea"));
        assert!(blocked.contains("fra"));
    }

    #[test]
    fn test_linux_clear_all() {
        let mock_exec = Box::new(MockCommandExecutor::new());
        let cmd_list = mock_exec.commands.clone();
        
        let firewall = LinuxFirewall::with_executor(mock_exec);
        let _ = firewall.clear_all();
        
        let cmds = cmd_list.lock().unwrap();
        assert!(cmds.iter().any(|c| c.contains("-F VSM_BLOCKS")));
        assert!(cmds.iter().any(|c| c.contains("-D OUTPUT -j VSM_BLOCKS")));
        assert!(cmds.iter().any(|c| c.contains("-X VSM_BLOCKS")));
    }
}
