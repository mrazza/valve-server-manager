use std::collections::HashSet;
use super::{FirewallDriver, CommandExecutor};

pub struct WindowsFirewall {
    executor: Box<dyn CommandExecutor>,
}

impl WindowsFirewall {
    #[cfg(not(test))]
    pub fn new() -> Self {
        use super::SystemCommandExecutor;
        Self::with_executor(Box::new(SystemCommandExecutor))
    }

    pub fn with_executor(executor: Box<dyn CommandExecutor>) -> Self {
        Self { executor }
    }
}

impl FirewallDriver for WindowsFirewall {
    fn block_pop(&self, pop: &str, ips: &[String]) -> Result<(), String> {
        // Remove existing rule to avoid duplicates
        let _ = self.unblock_pop(pop, ips);

        if ips.is_empty() {
            return Ok(());
        }

        // Join IPs with comma
        let remote_ip = ips.join(",");
        let rule_name = format!("VSM_Block_{}", pop);

        let status = self.executor.execute("netsh", &[
            "advfirewall", "firewall", "add", "rule",
            &format!("name={}", rule_name),
            "dir=out",
            "action=block",
            &format!("remoteip={}", remote_ip),
            "enable=yes",
        ])
        .map_err(|e| format!("Failed to run netsh: {}", e))?;

        if !status.status.success() {
            return Err(format!("netsh failed to create rule {}", rule_name));
        }

        Ok(())
    }

    fn unblock_pop(&self, pop: &str, _ips: &[String]) -> Result<(), String> {
        let rule_name = format!("VSM_Block_{}", pop);
        let _ = self.executor.execute("netsh", &[
            "advfirewall", "firewall", "delete", "rule",
            &format!("name={}", rule_name),
        ]);
        Ok(())
    }

    fn get_blocked_pops(&self) -> Result<HashSet<String>, String> {
        let output = self.executor.execute("netsh", &["advfirewall", "firewall", "show", "rule", "name=all"])
            .map_err(|e| format!("Failed to run netsh: {}", e))?;

        let rules_text = String::from_utf8_lossy(&output.stdout);
        let mut blocked = HashSet::new();

        for line in rules_text.lines() {
            if line.contains("VSM_Block_") {
                if let Some(pos) = line.find("VSM_Block_") {
                    let start = pos + "VSM_Block_".len();
                    // Extract alphanumeric pop code (e.g. sea, ams2, etc.)
                    let pop_code_part = &line[start..];
                    let pop_code: String = pop_code_part
                        .chars()
                        .take_while(|c| c.is_alphanumeric())
                        .collect();
                    
                    if !pop_code.is_empty() {
                        blocked.insert(pop_code);
                    }
                }
            }
        }

        Ok(blocked)
    }

    fn clear_all(&self) -> Result<(), String> {
        // Query blocked pops first
        if let Ok(blocked) = self.get_blocked_pops() {
            for pop in blocked {
                let _ = self.unblock_pop(&pop, &[]);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firewall::MockCommandExecutor;

    #[test]
    fn test_windows_block_pop() {
        let mock_exec = Box::new(MockCommandExecutor::new());
        let cmd_list = mock_exec.commands.clone();
        
        let firewall = WindowsFirewall::with_executor(mock_exec);
        
        let res = firewall.block_pop("sea", &["192.69.96.0/22".to_string(), "155.133.242.0/24".to_string()]);
        assert!(res.is_ok());
        
        let cmds = cmd_list.lock().unwrap();
        // Verify delete first then add
        assert!(cmds.iter().any(|c| c.contains("delete rule name=VSM_Block_sea")));
        assert!(cmds.iter().any(|c| c.contains("add rule name=VSM_Block_sea dir=out action=block remoteip=192.69.96.0/22,155.133.242.0/24 enable=yes")));
    }

    #[test]
    fn test_windows_get_blocked_pops() {
        let mock_exec = Box::new(MockCommandExecutor::new());
        *mock_exec.mock_stdout.lock().unwrap() = "Rule Name: VSM_Block_sea\nRule Name: VSM_Block_fra\n".to_string();
        
        let firewall = WindowsFirewall::with_executor(mock_exec);
        let blocked = firewall.get_blocked_pops().unwrap();
        
        assert_eq!(blocked.len(), 2);
        assert!(blocked.contains("sea"));
        assert!(blocked.contains("fra"));
    }
}
