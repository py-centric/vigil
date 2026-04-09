use crate::config::Config;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{backend::Backend, layout::Rect, Terminal};
use std::time::{Duration, Instant};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum ViewMode {
    #[default]
    Dashboard = 0,
    Topology = 1,
    Gantt = 2,
    RedisExplorer = 3,
    RedisStreams = 4,
    MongoExplorer = 5,
    KafkaViewer = 6,
    RabbitMQViewer = 7,
}

#[derive(Debug, Default)]
pub struct ViewStates {
    pub redis_explorer: [usize; 3],
    pub redis_streams: [usize; 2],
    pub mongo_explorer: [usize; 3],
    pub kafka_viewer: [usize; 2],
    pub rabbitmq_viewer: [usize; 2],
}

pub struct App {
    pub config: Config,
    pub mode: ViewMode,
    pub running: bool,
    pub filter_mode: bool,
    pub filter_input: String,
    pub tooltip: Option<String>,
    pub active_panel: usize,
    pub states: ViewStates,
    pub panel_areas: Vec<Rect>,
    pub tab_area: Option<Rect>,
    pub app_state: std::sync::Arc<std::sync::Mutex<crate::collectors::state::AppState>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let app_state = std::sync::Arc::new(std::sync::Mutex::new(crate::collectors::state::AppState::default()));
        
        // Start collectors
        crate::collectors::redis::start_redis_collector(config.redis.clone(), app_state.clone());
        crate::collectors::mongodb::start_mongodb_collector(config.mongodb.clone(), app_state.clone());
        crate::collectors::kafka::start_kafka_collector(config.kafka.clone(), app_state.clone());
        crate::collectors::rabbitmq::start_rabbitmq_collector(config.rabbitmq.clone(), app_state.clone());

        Self {
            config,
            mode: ViewMode::Dashboard,
            running: true,
            filter_mode: false,
            filter_input: String::new(),
            tooltip: None,
            active_panel: 0,
            states: ViewStates::default(),
            panel_areas: Vec::new(),
            tab_area: None,
            app_state,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|f| self.draw(f))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key),
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.on_tick();
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn on_tick(&mut self) {
        // Handle background polling dispatch or state updates here
        // E.g., read from collector channels into view caches
    }

    fn draw(&mut self, f: &mut ratatui::Frame) {
        super::views::draw_main(f, self);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if self.filter_mode {
            match key.code {
                KeyCode::Enter => self.filter_mode = false,
                KeyCode::Esc => {
                    self.filter_mode = false;
                    self.filter_input.clear();
                }
                KeyCode::Char(c) => self.filter_input.push(c),
                KeyCode::Backspace => {
                    self.filter_input.pop();
                }
                _ => {}
            }
            return;
        }

        // Global shortcuts
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            match key.code {
                KeyCode::Char('R') => { self.mode = ViewMode::RedisExplorer; self.active_panel = 0; }
                KeyCode::Char('M') => { self.mode = ViewMode::MongoExplorer; self.active_panel = 0; }
                KeyCode::Char('S') => { self.mode = ViewMode::RedisStreams; self.active_panel = 0; }
                KeyCode::Char('K') => { self.mode = ViewMode::KafkaViewer; self.active_panel = 0; }
                KeyCode::Char('O') => { self.mode = ViewMode::RabbitMQViewer; self.active_panel = 0; }
                KeyCode::Char('D') => { self.mode = ViewMode::Dashboard; self.active_panel = 0; }
                KeyCode::Char('T') => { self.mode = ViewMode::Topology; self.active_panel = 0; }
                KeyCode::Char('G') => { self.mode = ViewMode::Gantt; self.active_panel = 0; }
                KeyCode::BackTab => self.prev_panel(),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('t') => self.cycle_view(),
            KeyCode::Char('f') | KeyCode::Char('/') => self.filter_mode = true,
            KeyCode::Tab => self.next_panel(),
            KeyCode::Left | KeyCode::Char('h') => self.prev_panel(),
            KeyCode::Right | KeyCode::Char('l') => self.next_panel(),
            KeyCode::Up | KeyCode::Char('k') => self.prev_item(),
            KeyCode::Down | KeyCode::Char('j') => self.next_item(),
            KeyCode::Char('?') => {
                if self.tooltip.is_some() {
                    self.tooltip = None;
                } else {
                    self.tooltip = Some("Help:\n  Tab/Shift+Tab: switch focused panels\n  h/j/k/l or Arrows: navigate\n  Enter: Select\n  t: Cycle Views\n  q: Quit".to_string());
                }
            }
            KeyCode::Esc => {
                if self.tooltip.is_some() {
                    self.tooltip = None;
                }
            }
            _ => {
                // TODO: specific views input logic
            }
        }
    }

    fn cycle_view(&mut self) {
        self.mode = match self.mode {
            ViewMode::Dashboard => ViewMode::Topology,
            ViewMode::Topology => ViewMode::Gantt,
            ViewMode::Gantt => ViewMode::RedisExplorer,
            ViewMode::RedisExplorer => ViewMode::RedisStreams,
            ViewMode::RedisStreams => ViewMode::MongoExplorer,
            ViewMode::MongoExplorer => ViewMode::KafkaViewer,
            ViewMode::KafkaViewer => ViewMode::RabbitMQViewer,
            ViewMode::RabbitMQViewer => ViewMode::Dashboard,
        };
        self.active_panel = 0;
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            // Check if user clicked on Tabs
            if let Some(tab_a) = self.tab_area {
                // Click must be strictly inside the tab inner area. `Tabs` block has `Borders::ALL`.
                if mouse.row == tab_a.y + 1 {
                    let click_x = mouse.column.saturating_sub(tab_a.x + 1);
                    
                    let titles = vec![
                        "Dashboard", "Topology", "Gantt", "Redis", "Redis Streams",
                        "MongoDB", "Kafka", "RabbitMQ"
                    ];
                    
                    let mut current_x = 0;
                    for (i, title) in titles.iter().enumerate() {
                        let width = title.chars().count() as u16;
                        // Tab area: [current_x, current_x + width + 2] 
                        // The divider " | " is 3 chars, but we only have it BETWEEN titles.
                        // offset 0: Title1 (w1)
                        // offset w1: " | " (3)
                        // offset w1+3: Title2 (w2)
                        
                        if click_x >= current_x && click_x < current_x + width + (if i < titles.len() - 1 { 3 } else { 0 }) {
                            self.mode = match i {
                                0 => ViewMode::Dashboard,
                                1 => ViewMode::Topology,
                                2 => ViewMode::Gantt,
                                3 => ViewMode::RedisExplorer,
                                4 => ViewMode::RedisStreams,
                                5 => ViewMode::MongoExplorer,
                                6 => ViewMode::KafkaViewer,
                                7 | _ => ViewMode::RabbitMQViewer,
                            };
                            self.active_panel = 0;
                            return; // Handled
                        }
                        current_x += width + 3; 
                    }
                }
            }

            for i in 0..self.panel_areas.len() {
                let area = self.panel_areas[i];
                if mouse.column >= area.x && mouse.column < area.x + area.width &&
                   mouse.row >= area.y && mouse.row < area.y + area.height {
                    self.active_panel = i;
                    if let Some(state) = self.active_state_mut() {
                        let inner_y = mouse.row.saturating_sub(area.y).saturating_sub(1);
                        *state = inner_y as usize;
                    }
                    break;
                }
            }
        }
    }

    fn panel_count(&self) -> usize {
        match self.mode {
            ViewMode::Dashboard => 6,
            ViewMode::MongoExplorer | ViewMode::RedisExplorer => 3,
            ViewMode::RedisStreams | ViewMode::KafkaViewer | ViewMode::RabbitMQViewer => 2,
            ViewMode::Topology | ViewMode::Gantt => 1,
        }
    }

    fn next_panel(&mut self) {
        let count = self.panel_count();
        if count > 0 {
            self.active_panel = (self.active_panel + 1) % count;
        }
    }

    fn prev_panel(&mut self) {
        let count = self.panel_count();
        if count > 0 {
            self.active_panel = (self.active_panel + count - 1) % count;
        }
    }

    fn active_state_mut(&mut self) -> Option<&mut usize> {
        let panel = self.active_panel;
        match self.mode {
            ViewMode::RedisExplorer => Some(&mut self.states.redis_explorer[panel]),
            ViewMode::MongoExplorer => Some(&mut self.states.mongo_explorer[panel]),
            ViewMode::RedisStreams => Some(&mut self.states.redis_streams[panel]),
            ViewMode::KafkaViewer => Some(&mut self.states.kafka_viewer[panel]),
            ViewMode::RabbitMQViewer => Some(&mut self.states.rabbitmq_viewer[panel]),
            _ => None,
        }
    }

    fn next_item(&mut self) {
        if let Some(state) = self.active_state_mut() {
            *state = state.saturating_add(1);
        }
    }

    fn prev_item(&mut self) {
        if let Some(state) = self.active_state_mut() {
            *state = state.saturating_sub(1);
        }
    }
}
