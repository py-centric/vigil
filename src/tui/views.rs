use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Tabs},
    Frame,
};
use super::app::{App, ViewMode};

fn get_border_style(is_active: bool) -> Style {
    if is_active {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

fn item_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    }
}

pub fn draw_main(f: &mut Frame, app: &mut App) {
    let size = f.area();
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Statusbar
        ])
        .split(size);

    app.tab_area = Some(chunks[0]);
    app.panel_areas.clear();

    let titles = vec![
        "Dashboard", "Topology", "Gantt", "Redis", "Redis Streams",
        "MongoDB", "Kafka", "RabbitMQ"
    ];
    let tab_index = app.mode as usize;

    let tabs = Tabs::new(titles)
        .select(tab_index)
        .divider(" | ") // Explicitly set divider to 3 chars
        .block(Block::default().borders(Borders::ALL).title("Flux-Trace"))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    match app.mode {
        ViewMode::Dashboard => draw_dashboard(f, app, chunks[1]),
        ViewMode::Topology => draw_topology(f, app, chunks[1]),
        ViewMode::Gantt => draw_gantt(f, app, chunks[1]),
        ViewMode::RedisExplorer => draw_redis_explorer(f, app, chunks[1]),
        ViewMode::RedisStreams => draw_redis_streams(f, app, chunks[1]),
        ViewMode::MongoExplorer => draw_mongo_explorer(f, app, chunks[1]),
        ViewMode::KafkaViewer => draw_kafka_viewer(f, app, chunks[1]),
        ViewMode::RabbitMQViewer => draw_rabbitmq_viewer(f, app, chunks[1]),
    }

    let status_text = format!(
        " [Mode: {:?}] [Filter: {}] [Press '?' for Help] [Press 'q' to Quit] ",
        app.mode, app.filter_input
    );
    let statusbar = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(statusbar, chunks[2]);

    if let Some(ref tp) = app.tooltip {
        let popup_area = center(size, 40, 20); 
        f.render_widget(Clear, popup_area);
        
        let block = Block::default()
            .title(" Tooltip / Help ")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Yellow));
            
        let popup = Paragraph::new(tp.as_str()).block(block);
        f.render_widget(popup, popup_area);
    }
}

fn center(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
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

pub fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let state = app.app_state.lock().unwrap();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(6)]) // services vs logs
        .split(area);

    app.panel_areas = chunks.to_vec();

    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(chunks[0]);

    // 1. Kafka
    let kafka_title = Line::from(vec![Span::styled("Kafka ", Style::default().fg(Color::Cyan)), Span::raw("localhost:9092")]);
    let kafka_block = Block::default().borders(Borders::ALL).title(kafka_title).border_style(get_border_style(app.active_panel == 0));
    f.render_widget(kafka_block.clone(), top_row[0]);
    let k_inner = kafka_block.inner(top_row[0]);
    
    let k_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(4), Constraint::Min(2)]).split(k_inner);
    let (kafka_status, kafka_status_color) = if state.kafka_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let kafka_topics_count = state.kafka_topics.len().to_string();
    let kafka_text = vec![
        Line::from(vec![Span::raw("Status: "), Span::styled(kafka_status, Style::default().fg(kafka_status_color))]),
        Line::from(vec![Span::raw("Topics: "), Span::styled(kafka_topics_count, Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Msgs/min: "), Span::styled("13500", Style::default().fg(Color::Yellow))]),
    ];
    f.render_widget(Paragraph::new(kafka_text), k_layout[0]);
    let gauge = Gauge::default()
        .block(Block::default().title("Usage").borders(Borders::ALL))
        .gauge_style(Style::default().fg(if state.kafka_online { Color::Green } else { Color::DarkGray }))
        .percent(if state.kafka_online { 20 } else { 0 });
    f.render_widget(gauge, k_layout[1]);

    // 2. Redis
    let redis_title = Line::from(vec![Span::styled("Redis ", Style::default().fg(Color::Cyan)), Span::raw("localhost:6379")]);
    let (redis_status, redis_status_color) = if state.redis_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let mut total_keys = 0;
    for k in state.redis_keys.values() { 
        let keys_list: &Vec<(String, String)> = k;
        total_keys += keys_list.len(); 
    }
    let redis_text = vec![
        Line::from(vec![Span::raw("Status: "), Span::styled(redis_status, Style::default().fg(redis_status_color))]),
        Line::from(vec![Span::raw("DBs: "), Span::styled(state.redis_dbs.to_string(), Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Keys: "), Span::styled(total_keys.to_string(), Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Memory: "), Span::styled(&state.redis_mem, Style::default().fg(Color::Yellow))]),
    ];
    f.render_widget(Paragraph::new(redis_text).block(Block::default().borders(Borders::ALL).title(redis_title).border_style(get_border_style(app.active_panel == 0))), top_row[1]);

    // 3. RabbitMQ
    let rabbit_title = Line::from(vec![Span::styled("RabbitMQ ", Style::default().fg(Color::Cyan)), Span::raw("localhost:5672")]);
    let (rabbitmq_status, rabbit_color) = if state.rabbitmq_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let rabbit_text = vec![
        Line::from(vec![Span::raw("Status: "), Span::styled(rabbitmq_status, Style::default().fg(rabbit_color))]),
        Line::from(vec![Span::raw("Queues: "), Span::styled(state.rabbitmq_queues.len().to_string(), Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Msgs: "), Span::styled("225", Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Consumers: "), Span::styled("3", Style::default().fg(Color::Yellow))]),
    ];
    f.render_widget(Paragraph::new(rabbit_text).block(Block::default().borders(Borders::ALL).title(rabbit_title).border_style(get_border_style(app.active_panel == 0))), top_row[2]);

    // 4. MongoDB
    let mongo_title = Line::from(vec![Span::styled("MongoDB ", Style::default().fg(Color::Cyan)), Span::raw("localhost:27017")]);
    let (mongo_status, mongo_color) = if state.mongo_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let mongo_collections = state.mongo_collections.values().fold(0, |acc: usize, c: &Vec<String>| acc + c.len());
    let mongo_text = vec![
        Line::from(vec![Span::raw("Status: "), Span::styled(mongo_status, Style::default().fg(mongo_color))]),
        Line::from(vec![Span::raw("DBs: "), Span::styled(state.mongo_dbs.len().to_string(), Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Collections: "), Span::styled(mongo_collections.to_string(), Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("Size: "), Span::styled(&state.mongo_db_size, Style::default().fg(Color::Yellow))]),
    ];
    f.render_widget(Paragraph::new(mongo_text).block(Block::default().borders(Borders::ALL).title(mongo_title).border_style(get_border_style(app.active_panel == 0))), top_row[3]);

    // Logs
    let logs_text = if state.logs.is_empty() {
        "No logs yet. Logs will appear here...".to_string()
    } else {
        state.logs.join("\n")
    };
    let log_view = Paragraph::new(logs_text)
        .block(Block::default()
            .title(Span::styled("Logs", Style::default().fg(Color::Cyan)))
            .borders(Borders::ALL)
            .border_style(get_border_style(app.active_panel == 1)));
    f.render_widget(log_view, chunks[1]);
}

pub fn draw_redis_explorer(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let state = app.states.redis_explorer;
    let r_state = app.app_state.lock().unwrap();
    let db_count = if r_state.redis_dbs > 0 { r_state.redis_dbs } else { 16 };
    let mut dbs = vec![];
    for i in 0..db_count {
        let is_sel = state[0] == i;
        let prefix = if is_sel { "▶" } else { " " };
        dbs.push(Line::from(vec![Span::styled(format!("{} DB {}", prefix, i), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(dbs).block(Block::default().title("Databases").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let mut keys = vec![];
    let selected_db = state[0].min(db_count.saturating_sub(1));
    let db_keys = r_state.redis_keys.get(&selected_db).cloned().unwrap_or_default();
    
    for (i, (k, t)) in db_keys.iter().enumerate() {
        let is_sel = state[1] == i;
        keys.push(Line::from(vec![
            Span::styled(format!("{} ", k), item_style(is_sel)), 
            Span::styled(t.clone(), Style::default().fg(Color::DarkGray))
        ]));
    }
    if keys.is_empty() {
        keys.push(Line::from(Span::styled("No keys found", Style::default().fg(Color::DarkGray))));
    }
    f.render_widget(Paragraph::new(keys).block(Block::default().title("Keys").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);

    let val = if state[1] < db_keys.len() {
        let selected_key = &db_keys[state[1]].0;
        r_state.redis_vals.get(&(selected_db, selected_key.clone())).cloned().unwrap_or_else(|| "Loading...".to_string())
    } else {
        "Select a key".to_string()
    };
    f.render_widget(Paragraph::new(val).block(Block::default().title("Value").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 2))), chunks[2]);
}

pub fn draw_mongo_explorer(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let state = app.states.mongo_explorer;
    let m_state = app.app_state.lock().unwrap();
    
    let mut dbs = m_state.mongo_dbs.clone();
    if dbs.is_empty() { dbs.push("Loading...".to_string()); }
    
    let mut db_lines = vec![];
    for (i, db) in dbs.iter().enumerate() {
        let is_sel = state[0] == i;
        let prefix = if is_sel { "▶" } else { " " };
        db_lines.push(Line::from(vec![Span::styled(format!("{} {}", prefix, db), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(db_lines).block(Block::default().title("Databases").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let selected_db = state[0].min(dbs.len().saturating_sub(1));
    let db_name = dbs.get(selected_db).cloned().unwrap_or_default();
    
    let mut cols = m_state.mongo_collections.get(&db_name).cloned().unwrap_or_default();
    if cols.is_empty() { cols.push("None/Loading...".to_string()); }
    
    let mut col_lines = vec![];
    for (i, col) in cols.iter().enumerate() {
        let is_sel = state[1] == i;
        col_lines.push(Line::from(vec![Span::styled(col.clone(), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(col_lines).block(Block::default().title("Collections").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);

    let selected_col = state[1].min(cols.len().saturating_sub(1));
    let col_name = cols.get(selected_col).cloned().unwrap_or_default();
    
    let mut docs = m_state.mongo_docs.get(&(db_name, col_name)).cloned().unwrap_or_default();
    if docs.is_empty() { docs.push("No documents found".to_string()); }
    
    let mut doc_lines = vec![];
    for (i, doc) in docs.iter().enumerate() {
        let is_sel = state[2] == i;
        // if large document, we just show on multiple lines or it wraps. By default ratatui Paragraph wraps if we set it.
        doc_lines.push(Line::from(vec![Span::styled(doc.clone(), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(doc_lines).block(Block::default().title("Documents").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 2))), chunks[2]);
}

pub fn draw_redis_streams(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let ui_state = app.states.redis_streams;
    let data = app.app_state.lock().unwrap();

    let streams = if data.redis_streams.is_empty() { vec!["None/Loading...".to_string()] } else { data.redis_streams.clone() };
    let mut stream_lines = vec![];
    for (i, s) in streams.iter().enumerate() {
        stream_lines.push(Line::from(vec![Span::styled(s.as_str(), item_style(ui_state[0] == i))]));
    }
    f.render_widget(Paragraph::new(stream_lines).block(Block::default().title("Streams").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let selected_stream = streams.get(ui_state[0]).cloned().unwrap_or_default();
    let entries = data.redis_stream_entries.get(&selected_stream).cloned().unwrap_or_default();
    let mut entry_lines = vec![];
    for (i, e) in entries.iter().enumerate() {
        entry_lines.push(Line::from(vec![Span::styled(e.as_str(), item_style(ui_state[1] == i))]));
    }
    if entry_lines.is_empty() { entry_lines.push(Line::from(Span::styled("No entries found", Style::default().fg(Color::DarkGray)))); }
    f.render_widget(Paragraph::new(entry_lines).block(Block::default().title("Entries").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);
}

pub fn draw_kafka_viewer(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let ui_state = app.states.kafka_viewer;
    let data = app.app_state.lock().unwrap();

    let topics = if data.kafka_topics.is_empty() { vec!["None/Loading...".to_string()] } else { data.kafka_topics.clone() };
    let mut topic_lines = vec![];
    for (i, t) in topics.iter().enumerate() {
        topic_lines.push(Line::from(vec![Span::styled(t.as_str(), item_style(ui_state[0] == i))]));
    }
    f.render_widget(Paragraph::new(topic_lines).block(Block::default().title("Topics").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let selected_topic = topics.get(ui_state[0]).cloned().unwrap_or_default();
    let msgs = data.kafka_messages.get(&selected_topic).cloned().unwrap_or_default();
    let mut msg_lines = vec![];
    for (i, m) in msgs.iter().enumerate() {
        msg_lines.push(Line::from(vec![Span::styled(m.as_str(), item_style(ui_state[1] == i))]));
    }
    if msg_lines.is_empty() { msg_lines.push(Line::from(Span::styled("No messages found", Style::default().fg(Color::DarkGray)))); }
    f.render_widget(Paragraph::new(msg_lines).block(Block::default().title("Messages").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);
}

pub fn draw_rabbitmq_viewer(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let ui_state = app.states.rabbitmq_viewer;
    let data = app.app_state.lock().unwrap();

    let queues = if data.rabbitmq_queues.is_empty() { vec![("None/Loading...".to_string(), "".to_string())] } else { data.rabbitmq_queues.clone() };
    let mut queue_lines = vec![];
    for (i, (q, flag)) in queues.iter().enumerate() {
        queue_lines.push(Line::from(vec![
            Span::styled(format!("{} ", q), item_style(ui_state[0] == i)),
            Span::styled(flag.as_str(), Style::default().fg(Color::DarkGray))
        ]));
    }
    f.render_widget(Paragraph::new(queue_lines).block(Block::default().title("Queues").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let (selected_queue, _) = queues.get(ui_state[0]).cloned().unwrap_or_default();
    let msgs = data.rabbitmq_messages.get(&selected_queue).cloned().unwrap_or_default();
    let mut msg_lines = vec![];
    for (i, m) in msgs.iter().enumerate() {
        msg_lines.push(Line::from(vec![Span::styled(m.as_str(), item_style(ui_state[1] == i))]));
    }
    if msg_lines.is_empty() { msg_lines.push(Line::from(Span::styled("No messages found", Style::default().fg(Color::DarkGray)))); }
    f.render_widget(Paragraph::new(msg_lines).block(Block::default().title("Messages").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);
}

pub fn draw_topology(f: &mut Frame, app: &mut App, area: Rect) {
    app.panel_areas = vec![area];
    let p = Paragraph::new("Topology View - Graph nodes placeholder...").block(Block::default().title("Topology").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0)));
    f.render_widget(p, area);
}

pub fn draw_gantt(f: &mut Frame, app: &mut App, area: Rect) {
    app.panel_areas = vec![area];
    let p = Paragraph::new("Gantt Component View...").block(Block::default().title("Gantt").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0)));
    f.render_widget(p, area);
}
