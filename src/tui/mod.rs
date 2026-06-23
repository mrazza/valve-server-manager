pub mod ui;

use std::collections::{HashMap, HashSet};
use std::io;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::{ConfigManager, Settings};
use crate::sdr::{fetch_sdr_config, PoP};
use crate::firewall::{create_driver, FirewallDriver};
use crate::ping::Pinger;

pub struct AppState {
    pub pops: HashMap<String, PoP>,
    pub selectable_pops: Vec<(String, &'static str)>, // (pop_code, region_name)
    pub selected_index: usize,
    
    pub firewall: Box<dyn FirewallDriver>,
    pub config_manager: ConfigManager,
    pub settings: Settings,
    pub blocked_pops: HashSet<String>,
    
    pub pinger: Pinger,
    pub show_settings: bool,
    pub should_quit: bool,
    pub persist_rules_on_exit_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let config_manager = ConfigManager::new();
        let settings = config_manager.load();
        
        let pops = fetch_sdr_config();
        
        // Group and sort PoPs by geographic region
        let mut regions_group: HashMap<&str, Vec<String>> = HashMap::new();
        for code in pops.keys() {
            let region = crate::sdr::get_region_for_pop(code);
            regions_group.entry(region).or_default().push(code.clone());
        }
        
        let mut sorted_regions: Vec<&str> = regions_group.keys().copied().collect();
        sorted_regions.sort();
        
        let mut selectable_pops = Vec::new();
        for region in sorted_regions {
            let mut codes = regions_group.get(region).unwrap().clone();
            codes.sort();
            for code in codes {
                selectable_pops.push((code, region));
            }
        }

        let firewall = create_driver();
        
        // Query firewall to sync with system state
        let mut blocked_pops = firewall.get_blocked_pops().unwrap_or_default();
        
        // Also sync from saved settings if there is a discrepancy
        for saved_pop in &settings.blocked_pops {
            blocked_pops.insert(saved_pop.clone());
        }
        
        // Apply blocks to firewall to make sure they are active
        for pop_code in &blocked_pops {
            if let Some(pop) = pops.get(pop_code) {
                let ips: Vec<String> = pop.relays.as_ref()
                    .map(|r| r.iter().map(|relay| relay.ipv4.clone()).collect())
                    .unwrap_or_default();
                let _ = firewall.block_pop(pop_code, &ips);
            }
        }
        
        let pinger = Pinger::new(&pops);
        pinger.start();

        let persist_rules_on_exit_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(settings.persist_rules_on_exit));

        Ok(Self {
            pops,
            selectable_pops,
            selected_index: 0,
            firewall,
            config_manager,
            settings,
            blocked_pops,
            pinger,
            show_settings: false,
            should_quit: false,
            persist_rules_on_exit_flag,
        })
    }

    pub fn with_components(
        pops: HashMap<String, PoP>,
        firewall: Box<dyn FirewallDriver>,
        config_manager: ConfigManager,
        settings: Settings,
        pinger: Pinger,
    ) -> Self {
        let mut regions_group: HashMap<&str, Vec<String>> = HashMap::new();
        for code in pops.keys() {
            let region = crate::sdr::get_region_for_pop(code);
            regions_group.entry(region).or_default().push(code.clone());
        }
        
        let mut sorted_regions: Vec<&str> = regions_group.keys().copied().collect();
        sorted_regions.sort();
        
        let mut selectable_pops = Vec::new();
        for region in sorted_regions {
            let mut codes = regions_group.get(region).unwrap().clone();
            codes.sort();
            for code in codes {
                selectable_pops.push((code, region));
            }
        }

        let blocked_pops = settings.blocked_pops.iter().cloned().collect();
        let persist_rules_on_exit_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(settings.persist_rules_on_exit));

        Self {
            pops,
            selectable_pops,
            selected_index: 0,
            firewall,
            config_manager,
            settings,
            blocked_pops,
            pinger,
            show_settings: false,
            should_quit: false,
            persist_rules_on_exit_flag,
        }
    }

    pub fn toggle_selected(&mut self) {
        if self.selectable_pops.is_empty() {
            return;
        }

        let (pop_code, _) = &self.selectable_pops[self.selected_index];
        let pop = match self.pops.get(pop_code) {
            Some(p) => p,
            None => return,
        };

        let ips: Vec<String> = pop.relays.as_ref()
            .map(|r| r.iter().map(|relay| relay.ipv4.clone()).collect())
            .unwrap_or_default();

        if self.blocked_pops.contains(pop_code) {
            // Unblock
            if self.firewall.unblock_pop(pop_code, &ips).is_ok() {
                self.blocked_pops.remove(pop_code);
            }
        } else {
            // Block
            if self.firewall.block_pop(pop_code, &ips).is_ok() {
                self.blocked_pops.insert(pop_code.clone());
            }
        }

        // Sync with settings and save
        self.settings.blocked_pops = self.blocked_pops.iter().cloned().collect();
        let _ = self.config_manager.save(&self.settings);
    }

    pub fn unblock_all(&mut self) {
        let _ = self.firewall.clear_all();
        self.blocked_pops.clear();
        self.settings.blocked_pops.clear();
        let _ = self.config_manager.save(&self.settings);
    }

    pub fn toggle_persist_setting(&mut self) {
        self.settings.persist_rules_on_exit = !self.settings.persist_rules_on_exit;
        self.persist_rules_on_exit_flag.store(self.settings.persist_rules_on_exit, std::sync::atomic::Ordering::SeqCst);
        let _ = self.config_manager.save(&self.settings);
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::new().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Handle signal interrupts (Ctrl-C, SIGTERM)
    let persist_flag = state.persist_rules_on_exit_flag.clone();
    let _ = ctrlc::set_handler(move || {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        
        if !persist_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let firewall = create_driver();
            let _ = firewall.clear_all();
        }
        std::process::exit(0);
    });

    while !state.should_quit {
        terminal.draw(|f| ui::draw_ui(f, &mut state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if state.show_settings {
                                state.show_settings = false;
                            } else {
                                state.should_quit = true;
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            state.should_quit = true;
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            state.show_settings = !state.show_settings;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if !state.show_settings && !state.selectable_pops.is_empty() {
                                if state.selected_index > 0 {
                                    state.selected_index -= 1;
                                }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if !state.show_settings && !state.selectable_pops.is_empty() {
                                if state.selected_index < state.selectable_pops.len() - 1 {
                                    state.selected_index += 1;
                                }
                            }
                        }
                        KeyCode::Char(' ') | KeyCode::Enter => {
                            if state.show_settings {
                                state.toggle_persist_setting();
                            } else {
                                state.toggle_selected();
                            }
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            if !state.show_settings {
                                state.unblock_all();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Stop pinger background threads
    state.pinger.stop();

    // Clean up rules on exit if configured to not persist
    if !state.settings.persist_rules_on_exit {
        let _ = state.firewall.clear_all();
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firewall::{MockCommandExecutor, linux::LinuxFirewall};
    use crate::ping::Pinger;
    use std::sync::Arc;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_test_app() -> AppState {
        let mut pops = HashMap::new();
        pops.insert("sea".to_string(), crate::sdr::PoP {
            desc: "Seattle (Washington)".to_string(),
            aliases: None,
            relays: Some(vec![crate::sdr::Relay {
                ipv4: "192.69.96.0/22".to_string(),
                port_range: None,
            }]),
        });

        let mock_exec = Arc::new(MockCommandExecutor::new());
        let firewall = Box::new(LinuxFirewall::with_executor(Box::new(MockCommandExecutor::new())));
        
        let temp_file = std::env::temp_dir().join("vsm_test_tui_config.toml");
        let _ = std::fs::remove_file(&temp_file);
        let config_manager = ConfigManager::with_path(temp_file);
        let settings = Settings::default();

        let pinger = Pinger::with_executor(&pops, mock_exec);

        AppState::with_components(pops, firewall, config_manager, settings, pinger)
    }

    #[test]
    fn test_tui_state_toggle_and_reset() {
        let mut app = make_test_app();
        assert!(app.blocked_pops.is_empty());

        // Toggle selected (sea)
        app.toggle_selected();
        assert!(app.blocked_pops.contains("sea"));

        // Toggle again (unblock)
        app.toggle_selected();
        assert!(app.blocked_pops.is_empty());

        // Block and reset
        app.toggle_selected();
        assert!(app.blocked_pops.contains("sea"));
        app.unblock_all();
        assert!(app.blocked_pops.is_empty());
    }

    #[test]
    fn test_tui_rendering_normal() {
        let mut app = make_test_app();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();

        // Convert buffer to string for easy assertion checks
        let mut output_str = String::new();
        for y in 0..24 {
            for x in 0..80 {
                output_str.push(buffer.get(x, y).symbol().chars().next().unwrap_or(' '));
            }
            output_str.push('\n');
        }

        assert!(output_str.contains("VALVE SERVER MANAGER"));
        assert!(output_str.contains("Seattle"));
        assert!(output_str.contains("sea"));
        assert!(output_str.contains("ALLOWED"));
    }

    #[test]
    fn test_tui_rendering_settings_overlay() {
        let mut app = make_test_app();
        app.show_settings = true;
        
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| ui::draw_ui(f, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();

        let mut output_str = String::new();
        for y in 0..24 {
            for x in 0..80 {
                output_str.push(buffer.get(x, y).symbol().chars().next().unwrap_or(' '));
            }
            output_str.push('\n');
        }

        assert!(output_str.contains("Settings Configuration"));
        assert!(output_str.contains("Persist firewall rules on exit"));
    }

    #[test]
    fn test_app_state_new() {
        let app = AppState::new();
        assert!(app.is_ok());
    }
}
