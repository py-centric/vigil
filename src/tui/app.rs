use crate::collectors::state::AppState;
use crate::config::Config;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{backend::Backend, layout::Rect, Terminal};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const VIEW_TITLES: &[&str] = &[
    "Dashboard", "Topology", "Gantt", "Redis", "Redis Streams",
    "MongoDB", "Kafka", "RabbitMQ",
];

pub fn lock_state(state: &Arc<Mutex<AppState>>) -> MutexGuard<'_, AppState> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

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
    pub app_state: Arc<Mutex<AppState>>,
    pub expanded_doc: Option<String>,
    pub wrap_enabled: bool,
    pub expanded_doc_scroll: usize,
    pub dashboard_collapsed: [bool; 7],
}

impl App {
    pub fn new(config: Config) -> Self {
        let app_state = Arc::new(Mutex::new(AppState::default()));
        
        // Start collectors
        crate::collectors::redis::start_redis_collector(config.redis.clone(), app_state.clone());
        crate::collectors::mongodb::start_mongodb_collector(config.mongodb.clone(), app_state.clone());
        crate::collectors::kafka::start_kafka_collector(config.kafka.clone(), app_state.clone());
        crate::collectors::rabbitmq::start_rabbitmq_collector(config.rabbitmq.clone(), app_state.clone());
        crate::collectors::influxdb::start_influxdb_collector(config.influxdb.clone(), config.influxdb_token.clone(), app_state.clone());

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
            expanded_doc: None,
            wrap_enabled: false,
            expanded_doc_scroll: 0,
            dashboard_collapsed: [false; 7],
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
                    Event::Resize(_, _) => {
                        terminal.draw(|f| self.draw(f))?;
                    }
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
        Ok(())
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
                KeyCode::Char(c) if self.filter_input.len() < 64 => {
                    self.filter_input.push(c);
                },
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

        // If expanded doc is active, intercept navigation keys for scrolling
        if let Some(ref doc) = self.expanded_doc {
            let max_scroll = doc.lines().count().saturating_sub(1);
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.expanded_doc_scroll = self.expanded_doc_scroll.saturating_sub(1);
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.expanded_doc_scroll = self.expanded_doc_scroll.saturating_add(1).min(max_scroll);
                    return;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('c') => {
                if self.mode == ViewMode::Dashboard {
                    self.dashboard_collapsed[self.active_panel] = !self.dashboard_collapsed[self.active_panel];
                }
            }
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
                    self.tooltip = Some("Help:\n  Tab/Shift+Tab: switch focused panels\n  h/j/k/l or Arrows: navigate\n  c: Collapse dashboard panel\n  f or /: Filter items in list\n  Enter: Expand/collapse document (Mongo)\n  w: Toggle wrap mode\n  t: Cycle Views\n  q: Quit".to_string());
                }
            }
            KeyCode::Enter if self.mode == ViewMode::MongoExplorer && self.active_panel == 2 => {
                self.toggle_expand_doc();
            }
            KeyCode::Char('w') => {
                self.wrap_enabled = !self.wrap_enabled;
            }
            KeyCode::Esc => {
                if self.expanded_doc.is_some() {
                    self.expanded_doc = None;
                    self.expanded_doc_scroll = 0;
                } else if self.tooltip.is_some() {
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
                // Accept clicks on any row within the 3-row tab area
                if mouse.row >= tab_a.y && mouse.row < tab_a.y + tab_a.height {
                    let inner_x = mouse.column.saturating_sub(tab_a.x + 1);
                    let inner_width = tab_a.width.saturating_sub(2);
                    if inner_x >= inner_width {
                        return;
                    }
                    
                    let titles = VIEW_TITLES;
                    // Tabs widget renders each tab as:
                    //   padding_left(1) + title + padding_right(1) + divider(3) (no divider after last)
                    let mut current_x = 0u16;
                    for (i, title) in titles.iter().enumerate() {
                        let width = title.chars().count() as u16;
                        // Hit zone covers padding_left + title + padding_right
                        let tab_right = current_x + 1 + width + 1;

                        if inner_x >= current_x && inner_x < tab_right {
                            self.mode = match i {
                                0 => ViewMode::Dashboard,
                                1 => ViewMode::Topology,
                                2 => ViewMode::Gantt,
                                3 => ViewMode::RedisExplorer,
                                4 => ViewMode::RedisStreams,
                                5 => ViewMode::MongoExplorer,
                                6 => ViewMode::KafkaViewer,
                                _ => ViewMode::RabbitMQViewer,
                            };
                            self.active_panel = 0;
                            return;
                        }
                        // Advance past this tab + divider (if not last)
                        current_x = tab_right + if i < titles.len() - 1 { 3 } else { 0 };
                    }
                }
            }

            for i in 0..self.panel_areas.len() {
                let area = self.panel_areas[i];
                if mouse.column >= area.x && mouse.column < area.x + area.width &&
                   mouse.row >= area.y && mouse.row < area.y + area.height {
                    let is_mongo_docs = self.mode == ViewMode::MongoExplorer && i == 2;
                    self.active_panel = i;
                    if let Some(state) = self.active_state_mut() {
                        let inner_y = mouse.row.saturating_sub(area.y).saturating_sub(1);
                        // Mongo documents panel has separators between entries:
                        //   row 0: doc0, row 1: sep, row 2: doc1, row 3: sep, ...
                        // Adjust index and skip separator rows.
                        if is_mongo_docs && inner_y.is_multiple_of(2) {
                            *state = (inner_y as usize) / 2;
                        } else if !is_mongo_docs {
                            *state = inner_y as usize;
                        }
                    }
                    break;
                }
            }
        }
    }

    fn panel_count(&self) -> usize {
        match self.mode {
            ViewMode::Dashboard => 7,
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
            ViewMode::RedisExplorer => self.states.redis_explorer.get_mut(panel),
            ViewMode::MongoExplorer => self.states.mongo_explorer.get_mut(panel),
            ViewMode::RedisStreams => self.states.redis_streams.get_mut(panel),
            ViewMode::KafkaViewer => self.states.kafka_viewer.get_mut(panel),
            ViewMode::RabbitMQViewer => self.states.rabbitmq_viewer.get_mut(panel),
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

    fn toggle_expand_doc(&mut self) {
        if self.expanded_doc.is_some() {
            self.expanded_doc = None;
            return;
        }
        let state = self.states.mongo_explorer;
        let m_state = lock_state(&self.app_state);
        let dbs = m_state.mongo_dbs.clone();
        let selected_db = state[0].min(dbs.len().saturating_sub(1));
        let db_name = dbs.get(selected_db).cloned().unwrap_or_default();
        let cols = m_state.mongo_collections.get(&db_name).cloned().unwrap_or_default();
        let selected_col = state[1].min(cols.len().saturating_sub(1));
        let col_name = cols.get(selected_col).cloned().unwrap_or_default();
        let docs = m_state.mongo_docs.get(&(db_name, col_name)).cloned().unwrap_or_default();
        let doc_idx = state[2].min(docs.len().saturating_sub(1));
        if let Some(doc) = docs.get(doc_idx) {
            self.expanded_doc = Some(doc.clone());
            self.expanded_doc_scroll = 0;
        }
    }
}
