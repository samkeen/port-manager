mod config;

use std::process::Command;
use std::io;
use std::time::Duration;

use anyhow::{Result, Context};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui::style::{Color, Style, Stylize};
use sysinfo::System;

use config::Config;

/// CLI tool to manage processes running on ports
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Show processes running on this specific port
    #[arg(short, long)]
    port: Option<u16>,
}

/// Represents a process running on a port
struct PortProcess {
    pid: u32,
    name: String,
    port: u16,
    command: String,
}

/// UI view states
enum View {
    ProcessList,
    FilterManagement,
}

/// Application state
struct App {
    port_processes: Vec<PortProcess>,
    selected_idx: Option<usize>,
    should_quit: bool,
    config: Config,
    current_view: View,
    filter_selected_idx: Option<usize>,
    show_add_filter_popup: bool,
    add_filter_input: String,
}

impl App {
    fn new() -> Result<Self> {
        Ok(Self {
            port_processes: Vec::new(),
            selected_idx: None,
            should_quit: false,
            config: Config::load()?,
            current_view: View::ProcessList,
            filter_selected_idx: None,
            show_add_filter_popup: false,
            add_filter_input: String::new(),
        })
    }

    /// Reload process list
    fn refresh_processes(&mut self) -> Result<()> {
        let all_processes = get_port_processes()?;
        
        // Filter processes based on configuration
        self.port_processes = all_processes.into_iter()
            .filter(|process| {
                // Check if the port is within range
                let port_in_range = process.port >= self.config.min_port && 
                                    process.port <= self.config.max_port;
                
                // Check if the process name is in the filter list
                let name_not_filtered = !self.config.filtered_process_names
                    .iter()
                    .any(|filtered| process.name.contains(filtered));
                
                port_in_range && name_not_filtered
            })
            .collect();
        
        // Update process list selection
        if !self.port_processes.is_empty() && self.selected_idx.is_none() {
            self.selected_idx = Some(0);
        }
        
        // Update filter list selection if in filter view
        if !self.config.filtered_process_names.is_empty() && self.filter_selected_idx.is_none() {
            self.filter_selected_idx = Some(0);
        }
        
        Ok(())
    }
    
    /// Toggle between views
    fn toggle_view(&mut self) {
        match self.current_view {
            View::ProcessList => self.current_view = View::FilterManagement,
            View::FilterManagement => self.current_view = View::ProcessList,
        }
    }
    
    /// Toggle add filter popup
    fn toggle_add_filter_popup(&mut self) {
        self.show_add_filter_popup = !self.show_add_filter_popup;
        if !self.show_add_filter_popup {
            self.add_filter_input.clear();
        }
    }
    
    /// Add character to filter input
    fn add_char_to_filter(&mut self, c: char) {
        self.add_filter_input.push(c);
    }
    
    /// Delete character from filter input
    fn delete_char_from_filter(&mut self) {
        self.add_filter_input.pop();
    }
    
    /// Save the current filter input
    fn save_filter(&mut self) -> Result<()> {
        let filter = self.add_filter_input.trim().to_string();
        if !filter.is_empty() {
            self.config.add_filtered_process(filter)?;
            self.refresh_processes()?;
        }
        self.toggle_add_filter_popup();
        Ok(())
    }
    
    /// Add current process to filter list
    fn filter_selected_process(&mut self) -> Result<()> {
        if let Some(selected) = self.selected_idx {
            if let Some(process) = self.port_processes.get(selected) {
                let process_name = process.name.clone();
                self.config.add_filtered_process(process_name)?;
                self.refresh_processes()?;
            }
        }
        Ok(())
    }

    /// Move selection up
    fn previous(&mut self) {
        match self.current_view {
            View::ProcessList => {
                if let Some(selected) = self.selected_idx {
                    if selected > 0 {
                        self.selected_idx = Some(selected - 1);
                    }
                }
            },
            View::FilterManagement => {
                if let Some(selected) = self.filter_selected_idx {
                    if selected > 0 {
                        self.filter_selected_idx = Some(selected - 1);
                    }
                }
            }
        }
    }

    /// Move selection down
    fn next(&mut self) {
        match self.current_view {
            View::ProcessList => {
                if let Some(selected) = self.selected_idx {
                    if selected < self.port_processes.len().saturating_sub(1) {
                        self.selected_idx = Some(selected + 1);
                    }
                }
            },
            View::FilterManagement => {
                if let Some(selected) = self.filter_selected_idx {
                    if selected < self.config.filtered_process_names.len().saturating_sub(1) {
                        self.filter_selected_idx = Some(selected + 1);
                    }
                }
            }
        }
    }

    /// Kill selected process
    fn kill_selected(&mut self) -> Result<()> {
        match self.current_view {
            View::ProcessList => {
                if let Some(selected) = self.selected_idx {
                    if let Some(process) = self.port_processes.get(selected) {
                        kill_process(process.pid)?;
                        
                        // Refresh the process list
                        self.refresh_processes()?;
                        
                        // Adjust selection if needed
                        if self.port_processes.is_empty() {
                            self.selected_idx = None;
                        } else if selected >= self.port_processes.len() {
                            self.selected_idx = Some(self.port_processes.len() - 1);
                        }
                    }
                }
            },
            View::FilterManagement => {
                // In filter management view, remove the selected filter
                if let Some(selected) = self.filter_selected_idx {
                    if let Some(filter_name) = self.config.filtered_process_names.get(selected) {
                        let filter_name = filter_name.clone();
                        self.config.remove_filtered_process(&filter_name)?;
                        
                        // Adjust selection if needed
                        if self.config.filtered_process_names.is_empty() {
                            self.filter_selected_idx = None;
                        } else if selected >= self.config.filtered_process_names.len() {
                            self.filter_selected_idx = Some(self.config.filtered_process_names.len() - 1);
                        }
                        
                        // Refresh process list with updated filters
                        self.refresh_processes()?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Get list of processes running on ports
fn get_port_processes() -> Result<Vec<PortProcess>> {
    let mut port_processes = Vec::new();
    
    // On macOS, use `lsof` to find processes listening on ports
    let output = Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output()
        .context("Failed to execute lsof command")?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("lsof command failed"));
    }
    
    let output_str = String::from_utf8(output.stdout)
        .context("Failed to parse lsof output as UTF-8")?;
    
    // Load system info to get process details
    let mut system = System::new();
    system.refresh_processes();
    
    // Skip the header line
    for line in output_str.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 9 {
            let process_name = parts[0].to_string();
            let pid_str = parts[1];
            
            // Extract port from address (format is typically like: *:8080)
            let addr_port = parts[8];
            if let Some(port_str) = addr_port.split(':').last() {
                if let (Ok(pid), Ok(port)) = (pid_str.parse::<u32>(), port_str.parse::<u16>()) {
                    let command = {
                        // Get command info via ps command
                        let cmd_output = Command::new("ps")
                            .args(["-o", "command=", "-p", &pid.to_string()])
                            .output();
                        
                        if let Ok(output) = cmd_output {
                            String::from_utf8_lossy(&output.stdout).trim().to_string()
                        } else {
                            String::new()
                        }
                    };
                    
                    port_processes.push(PortProcess {
                        pid,
                        name: process_name,
                        port,
                        command,
                    });
                }
            }
        }
    }
    
    // Sort by port number
    port_processes.sort_by_key(|p| p.port);
    
    Ok(port_processes)
}

/// Kill a process by PID
fn kill_process(pid: u32) -> Result<()> {
    let output = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .context("Failed to execute kill command")?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to kill process: {}", error));
    }
    
    Ok(())
}

/// Initialize the terminal for TUI
fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    io::stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend).context("Failed to create terminal")?;
    Ok(terminal)
}

/// Restore terminal to original state
fn restore_terminal() -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    io::stdout()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    Ok(())
}

/// Main UI rendering function
fn ui(frame: &mut Frame, app: &App) {
    // Render the current view
    match app.current_view {
        View::ProcessList => render_process_view(frame, app),
        View::FilterManagement => render_filter_view(frame, app),
    }
    
    // Render the add filter popup if active
    if app.show_add_filter_popup {
        render_add_filter_popup(frame, app);
    }
}

/// Render the process list view
fn render_process_view(frame: &mut Frame, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Table
            Constraint::Length(1),  // Spacing
            Constraint::Length(3),  // Help
        ])
        .split(frame.size());
    
    // Title block
    let title_block = Block::default()
        .title("Port Manager")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    
    let title_text = Paragraph::new("Monitor and manage processes running on ports")
        .block(title_block)
        .alignment(Alignment::Center);
    
    frame.render_widget(title_text, layout[0]);
    
    // Process table
    let headers = vec!["PID", "Port", "Process Name", "Command"];
    let header_cells = headers
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().bold()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);
    
    let rows = app.port_processes.iter().map(|process| {
        let cells = vec![
            Cell::from(process.pid.to_string()),
            Cell::from(process.port.to_string()),
            Cell::from(process.name.clone()),
            Cell::from(process.command.clone()),
        ];
        Row::new(cells).height(1)
    });
    
    let table = Table::new(
        rows,
        [   
            Constraint::Length(10),     // PID
            Constraint::Length(10),     // Port
            Constraint::Length(20),     // Process Name
            Constraint::Percentage(60),  // Command
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Processes")
    )
    .highlight_style(
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    )
    .highlight_symbol(">> ");
    
    // Render table with selection
    let table_state = &mut TableState::default().with_selected(app.selected_idx);
    frame.render_stateful_widget(table, layout[1], table_state);
    
    // Help text
    let help_text = "↑/↓: Navigate | Enter/k: Kill process | f: Filter process | F: Manage filters | r: Refresh | q: Quit";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
        )
        .alignment(Alignment::Center);
    
    frame.render_widget(help, layout[3]);
}

/// Render the filter management view
fn render_filter_view(frame: &mut Frame, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Filter list
            Constraint::Length(1),  // Spacing
            Constraint::Length(3),  // Help
        ])
        .split(frame.size());
    
    // Title block
    let title_block = Block::default()
        .title("Process Filters")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    
    let title_text = Paragraph::new("Manage process name filters")
        .block(title_block)
        .alignment(Alignment::Center);
    
    frame.render_widget(title_text, layout[0]);
    
    // Filter list
    let filters = app.config.filtered_process_names.iter().enumerate()
        .map(|(i, name)| {
            ListItem::new(format!("{}. {}", i + 1, name))
        })
        .collect::<Vec<_>>();
    
    let filter_list = List::new(filters)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Filtered Process Names")
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");
    
    let mut filter_state = ListState::default();
    filter_state.select(app.filter_selected_idx);
    
    frame.render_stateful_widget(filter_list, layout[1], &mut filter_state);
    
    // Help text
    let help_text = "↑/↓: Navigate | Enter/Delete: Remove filter | a: Add new filter | F: Return to processes | q: Quit";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
        )
        .alignment(Alignment::Center);
    
    frame.render_widget(help, layout[3]);
}

/// Render a popup for adding a new filter
fn render_add_filter_popup(frame: &mut Frame, app: &App) {
    let popup_area = centered_rect(60, 20, frame.size());
    
    // Clear the area
    frame.render_widget(Clear, popup_area);
    
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(3),  // Input
            Constraint::Length(3),  // Help
        ])
        .split(popup_area);
    
    // Title
    let title = Paragraph::new("Add Process Filter")
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
        );
    
    frame.render_widget(title, popup_layout[0]);
    
    // Input
    let input = Paragraph::new(app.add_filter_input.as_str())
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Process Name")
        );
    
    frame.render_widget(input, popup_layout[1]);
    
    // Place cursor at the end of input
    frame.set_cursor(
        popup_layout[1].x + app.add_filter_input.len() as u16 + 1,
        popup_layout[1].y + 1,
    );
    
    // Help
    let help = Paragraph::new("Enter: Save | Esc: Cancel")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
        );
    
    frame.render_widget(help, popup_layout[2]);
}

/// Helper function to create a centered rect using up certain percentage of the available rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    // Initial refresh
    app.refresh_processes()?;
    
    loop {
        terminal.draw(|frame| ui(frame, app))?;
        
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle popup first if it's active
                    if app.show_add_filter_popup {
                        match key.code {
                            KeyCode::Esc => {
                                app.toggle_add_filter_popup();
                            }
                            KeyCode::Char(c) => {
                                app.add_char_to_filter(c);
                            }
                            KeyCode::Backspace => {
                                app.delete_char_from_filter();
                            }
                            KeyCode::Enter => {
                                app.save_filter()?;
                            }
                            _ => {}
                        }
                    } else {
                        match app.current_view {
                            View::ProcessList => match key.code {
                                KeyCode::Char('q') => {
                                    app.should_quit = true;
                                }
                                KeyCode::Char('r') => {
                                    app.refresh_processes()?;
                                }
                                KeyCode::Char('f') => {
                                    app.filter_selected_process()?;
                                }
                                KeyCode::Char('F') => {
                                    app.toggle_view();
                                }
                                KeyCode::Up => {
                                    app.previous();
                                }
                                KeyCode::Down => {
                                    app.next();
                                }
                                KeyCode::Enter | KeyCode::Char('k') => {
                                    app.kill_selected()?;
                                }
                                _ => {}
                            },
                            View::FilterManagement => match key.code {
                                KeyCode::Char('q') => {
                                    app.should_quit = true;
                                }
                                KeyCode::Char('a') => {
                                    app.toggle_add_filter_popup();
                                }
                                KeyCode::Char('F') => {
                                    app.toggle_view();
                                }
                                KeyCode::Up => {
                                    app.previous();
                                }
                                KeyCode::Down => {
                                    app.next();
                                }
                                KeyCode::Enter | KeyCode::Delete => {
                                    app.kill_selected()?;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        
        if app.should_quit {
            break;
        }
    }
    
    Ok(())
}

fn main() -> Result<()> {
    // Setup logging
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let _args = Args::parse();
    
    // Initialize terminal
    let mut terminal = init_terminal()?;
    
    // Create app state
    let mut app = App::new()?;
    
    // Run the application
    let result = run_app(&mut terminal, &mut app);
    
    // Ensure terminal is restored even if there's an error
    restore_terminal()?;
    
    // Return the result from running the app
    result
}
