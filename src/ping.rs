use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::firewall::{CommandExecutor, SystemCommandExecutor};

pub struct Pinger {
    ping_results: Arc<Mutex<HashMap<String, Option<u16>>>>,
    pop_ips: HashMap<String, String>, // PoP Code -> IP Address
    running: Arc<Mutex<bool>>,
    executor: Arc<dyn CommandExecutor>,
}

impl Pinger {
    pub fn new(pops: &HashMap<String, crate::sdr::PoP>) -> Self {
        Self::with_executor(pops, Arc::new(SystemCommandExecutor))
    }

    pub fn with_executor(pops: &HashMap<String, crate::sdr::PoP>, executor: Arc<dyn CommandExecutor>) -> Self {
        let mut pop_ips = HashMap::new();
        let mut ping_results = HashMap::new();

        for (code, pop) in pops {
            if let Some(relays) = &pop.relays {
                if let Some(first_relay) = relays.first() {
                    let ip = first_relay.ipv4.split('/').next().unwrap_or(&first_relay.ipv4).to_string();
                    pop_ips.insert(code.clone(), ip);
                    ping_results.insert(code.clone(), None);
                }
            }
        }

        Self {
            ping_results: Arc::new(Mutex::new(ping_results)),
            pop_ips,
            running: Arc::new(Mutex::new(false)),
            executor,
        }
    }

    pub fn start(&self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;

        let results = self.ping_results.clone();
        let pop_ips = self.pop_ips.clone();
        let running_flag = self.running.clone();
        let executor = self.executor.clone();

        thread::spawn(move || {
            loop {
                // Check if we should stop
                {
                    let r = running_flag.lock().unwrap();
                    if !*r {
                        break;
                    }
                }

                let pops_list: Vec<(String, String)> = pop_ips
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Ping in small batches of 5 to avoid resource exhaustion
                let chunk_size = 5;
                for chunk in pops_list.chunks(chunk_size) {
                    // Double check if we were stopped mid-cycle
                    {
                        let r = running_flag.lock().unwrap();
                        if !*r {
                            break;
                        }
                    }

                    let mut handles = Vec::new();
                    for (code, ip) in chunk {
                        let code = code.clone();
                        let ip = ip.clone();
                        let results = results.clone();
                        let executor = executor.clone();
                        
                        handles.push(thread::spawn(move || {
                            let ping_val = ping_ip_with_executor(&ip, &*executor);
                            let mut res = results.lock().unwrap();
                            res.insert(code, ping_val);
                        }));
                    }
                    
                    // Wait for batch to complete
                    for handle in handles {
                        let _ = handle.join();
                    }

                    // Stagger batches slightly
                    thread::sleep(Duration::from_millis(50));
                }

                // Wait 15 seconds before the next latency refresh
                for _ in 0..150 {
                    thread::sleep(Duration::from_millis(100));
                    let r = running_flag.lock().unwrap();
                    if !*r {
                        return;
                    }
                }
            }
        });
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    pub fn get_results(&self) -> HashMap<String, Option<u16>> {
        self.ping_results.lock().unwrap().clone()
    }
}

fn ping_ip_with_executor(ip: &str, executor: &dyn CommandExecutor) -> Option<u16> {
    #[cfg(windows)]
    let output = executor.execute("ping", &["-n", "1", "-w", "1000", ip]);

    #[cfg(not(windows))]
    let output = executor.execute("ping", &["-c", "1", "-W", "1", ip]);

    let output = match output {
        Ok(out) => out,
        Err(_) => return None,
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ping_time(&stdout)
}

fn parse_ping_time(stdout: &str) -> Option<u16> {
    if let Some(pos) = stdout.find("time=") {
        let start = pos + "time=".len();
        let val_part = &stdout[start..];
        
        let mut num_str = String::new();
        for c in val_part.chars() {
            if c.is_ascii_digit() || c == '.' {
                num_str.push(c);
            } else {
                break;
            }
        }

        if let Ok(val) = num_str.parse::<f32>() {
            return Some(val.round() as u16);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firewall::MockCommandExecutor;

    #[test]
    fn test_parse_ping_time_linux() {
        let sample = "64 bytes from 155.133.248.36: icmp_seq=1 ttl=57 time=15.4 ms";
        assert_eq!(parse_ping_time(sample), Some(15));
    }

    #[test]
    fn test_parse_ping_time_windows() {
        let sample = "Reply from 155.133.248.36: bytes=32 time=42ms TTL=57";
        assert_eq!(parse_ping_time(sample), Some(42));
    }

    #[test]
    fn test_pinger_background_execution() {
        let mut pops = HashMap::new();
        pops.insert("sea".to_string(), crate::sdr::PoP {
            desc: "Seattle".to_string(),
            aliases: None,
            relays: Some(vec![crate::sdr::Relay {
                ipv4: "192.69.96.0/22".to_string(),
                port_range: None,
            }]),
        });

        let mock_exec = Arc::new(MockCommandExecutor::new());
        *mock_exec.mock_stdout.lock().unwrap() = "Reply from 192.69.96.0: bytes=32 time=12ms TTL=57".to_string();

        let pinger = Pinger::with_executor(&pops, mock_exec);
        pinger.start();

        // Wait a bit for thread execution
        thread::sleep(Duration::from_millis(150));
        pinger.stop();

        let results = pinger.get_results();
        assert_eq!(results.get("sea").copied().flatten(), Some(12));
    }
}
