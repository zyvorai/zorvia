// TUI Application - Main application state machine and event loop

use super::config::TuiConfig;
use super::state::AppState;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::Backend,
    Terminal,
};
use std::time::Duration;

/// TUI View enum - different screens in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    VmList,
    VmDetails,
    Snapshots,
    Profiles,
    Blueprints,
    Help,
}

/// Main TUI Application
pub struct App {
    /// Should the application quit
    pub should_quit: bool,

    /// Current view
    pub current_view: View,

    /// Application state
    pub state: AppState,

    /// TUI configuration
    pub config: TuiConfig,

    /// Error message to display
    pub error_message: Option<String>,

    /// Success message to display
    pub success_message: Option<String>,
}

impl App {
    /// Create a new TUI application
    pub fn new(namespace: String) -> Result<Self> {
        let config = TuiConfig::load()?;
        let state = AppState::new(namespace);

        Ok(Self {
            should_quit: false,
            current_view: View::Dashboard,
            state,
            config,
            error_message: None,
            success_message: None,
        })
    }

    /// Create a new TUI application with custom config
    pub fn with_config(namespace: String, config: TuiConfig) -> Self {
        let state = AppState::new(namespace);

        Self {
            should_quit: false,
            current_view: View::Dashboard,
            state,
            config,
            error_message: None,
            success_message: None,
        }
    }

    /// Run the TUI application event loop
    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Initial data load
        self.refresh_data().await?;

        loop {
            // Draw the UI
            terminal.draw(|f| self.render(f))?;

            // Handle events with timeout
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key).await?;
                }
            }

            // Check if we should quit
            if self.should_quit {
                break;
            }

            // Auto-refresh data every 5 seconds
            if self.state.should_refresh() {
                if let Err(e) = self.refresh_data().await {
                    self.error_message = Some(format!("Failed to refresh: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Render the current view
    fn render(&mut self, f: &mut ratatui::Frame) {
        use super::ui;

        match self.current_view {
            View::Dashboard => ui::dashboard::render(f, &self.state, &self.config),
            View::VmList => ui::vm_list::render(f, &mut self.state, &self.config),
            View::VmDetails => ui::vm_details::render(f, &self.state, &self.config),
            View::Snapshots => ui::snapshots::render(f, &self.state, &self.config),
            View::Profiles => ui::profiles::render(f, &self.state, &self.config),
            View::Blueprints => ui::blueprints::render(f, &self.state, &self.config),
            View::Help => ui::help::render(f, &self.config),
        }

        // Render status messages if present
        self.render_messages(f);
    }

    /// Render status messages (errors, success)
    fn render_messages(&mut self, f: &mut ratatui::Frame) {
        use ratatui::{
            layout::{Alignment, Constraint, Direction, Layout, Rect},
            style::{Color, Modifier, Style},
            widgets::{Block, Borders, Paragraph},
        };

        let area = f.size();

        // Show error message if present
        if let Some(ref msg) = self.error_message {
            let error_area = Rect {
                x: area.width / 4,
                y: area.height / 3,
                width: area.width / 2,
                height: 3,
            };

            let error = Paragraph::new(msg.as_str())
                .block(Block::default().borders(Borders::ALL).title("Error"))
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);

            f.render_widget(error, error_area);
        }

        // Show success message if present
        if let Some(ref msg) = self.success_message {
            let success_area = Rect {
                x: area.width / 4,
                y: area.height / 3,
                width: area.width / 2,
                height: 3,
            };

            let success = Paragraph::new(msg.as_str())
                .block(Block::default().borders(Borders::ALL).title("Success"))
                .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);

            f.render_widget(success, success_area);
        }
    }

    /// Handle keyboard input
    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear status messages on any key
        self.error_message = None;
        self.success_message = None;

        // Global keybindings (work in all views)
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                return Ok(());
            }
            KeyCode::Char('?') => {
                self.current_view = View::Help;
                return Ok(());
            }
            KeyCode::Char('1') => {
                self.current_view = View::Dashboard;
                return Ok(());
            }
            KeyCode::Char('2') => {
                self.current_view = View::VmList;
                return Ok(());
            }
            KeyCode::Char('3') => {
                self.current_view = View::Snapshots;
                return Ok(());
            }
            KeyCode::Char('4') => {
                self.current_view = View::Profiles;
                return Ok(());
            }
            KeyCode::Char('5') => {
                self.current_view = View::Blueprints;
                return Ok(());
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh_data().await?;
                self.success_message = Some("Data refreshed".to_string());
                return Ok(());
            }
            _ => {}
        }

        // View-specific keybindings
        match self.current_view {
            View::VmList => self.handle_vm_list_key(key).await?,
            View::Dashboard => self.handle_dashboard_key(key).await?,
            View::VmDetails => self.handle_vm_details_key(key).await?,
            View::Snapshots => self.handle_snapshots_key(key).await?,
            _ => {}
        }

        Ok(())
    }

    /// Handle VM list view keys
    async fn handle_vm_list_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.select_next();
            }
            KeyCode::Enter => {
                self.current_view = View::VmDetails;
            }
            KeyCode::Char('s') => {
                if let Some(vm) = self.state.selected_vm() {
                    let vm_name = vm.name.clone();
                    self.start_vm(&vm_name).await?;
                }
            }
            KeyCode::Char('x') => {
                if let Some(vm) = self.state.selected_vm() {
                    let vm_name = vm.name.clone();
                    self.stop_vm(&vm_name).await?;
                }
            }
            KeyCode::Char('d') => {
                if let Some(vm) = self.state.selected_vm() {
                    let vm_name = vm.name.clone();
                    self.delete_vm(&vm_name).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle dashboard view keys
    async fn handle_dashboard_key(&mut self, _key: KeyEvent) -> Result<()> {
        // Dashboard is mostly read-only
        Ok(())
    }

    /// Handle VM details view keys
    async fn handle_vm_details_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Backspace => {
                self.current_view = View::VmList;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle snapshots view keys
    async fn handle_snapshots_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.select_previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.select_next();
            }
            _ => {}
        }
        Ok(())
    }

    /// Refresh all data from Kubernetes
    async fn refresh_data(&mut self) -> Result<()> {
        self.state.refresh_vms().await?;
        self.state.refresh_snapshots().await?;
        Ok(())
    }

    /// Start a VM
    async fn start_vm(&mut self, vm_name: &str) -> Result<()> {
        use crate::kube::KubeClient;

        let client = KubeClient::new().await?;
        client.start_vm(&self.state.namespace, vm_name).await?;

        self.success_message = Some(format!("Started VM: {}", vm_name));
        self.refresh_data().await?;

        Ok(())
    }

    /// Stop a VM
    async fn stop_vm(&mut self, vm_name: &str) -> Result<()> {
        use crate::kube::KubeClient;

        let client = KubeClient::new().await?;
        client.stop_vm(&self.state.namespace, vm_name).await?;

        self.success_message = Some(format!("Stopped VM: {}", vm_name));
        self.refresh_data().await?;

        Ok(())
    }

    /// Delete a VM
    async fn delete_vm(&mut self, vm_name: &str) -> Result<()> {
        use crate::kube::KubeClient;

        let client = KubeClient::new().await?;
        client.delete_vm(&self.state.namespace, vm_name).await?;

        self.success_message = Some(format!("Deleted VM: {}", vm_name));
        self.refresh_data().await?;

        Ok(())
    }
}
