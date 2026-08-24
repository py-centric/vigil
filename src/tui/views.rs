use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
    Frame,
};
use super::app::App;
use super::app::{lock_state, ViewMode, VIEW_TITLES};

fn highlight_json_line(line: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut remaining = line;
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find('"') {
            if pos > 0 {
                let before = &remaining[..pos];
                let mut buf = String::new();
                for ch in before.chars() {
                    if ch == ':' {
                        spans.push(token_span(&buf));
                        buf.clear();
                        spans.push(Span::raw(":"));
                    } else if ch == ',' {
                        spans.push(token_span(&buf));
                        buf.clear();
                        spans.push(Span::raw(","));
                    } else {
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    spans.push(token_span(&buf));
                }
                remaining = &remaining[pos..];
            }
            let rest = &remaining[1..];
            let quote_end = rest.find('"').map(|p| p + 2).unwrap_or(remaining.len());
            let s = &remaining[..quote_end];
            let is_key = remaining[quote_end..].starts_with(':');
            spans.push(Span::styled(
                s.to_string(),
                Style::default().fg(if is_key { Color::Cyan } else { Color::Green }),
            ));
            remaining = &remaining[quote_end..];
        } else {
            let mut buf = String::new();
            for ch in remaining.chars() {
                if ch == ':' {
                    spans.push(token_span(&buf));
                    buf.clear();
                    spans.push(Span::raw(":"));
                } else if ch == ',' {
                    spans.push(token_span(&buf));
                    buf.clear();
                    spans.push(Span::raw(","));
                } else {
                    buf.push(ch);
                }
            }
            if !buf.is_empty() {
                spans.push(token_span(&buf));
            }
            break;
        }
    }
    Line::from(spans)
}

fn token_span(token: &str) -> Span<'static> {
    let t = token.trim();
    if t.is_empty() {
        return Span::raw(token.to_string());
    }
    if t == "true" || t == "false" {
        Span::styled(t.to_string(), Style::default().fg(Color::Magenta))
    } else if t == "null" {
        Span::styled("null".to_string(), Style::default().fg(Color::DarkGray))
    } else if t.parse::<f64>().is_ok() {
        Span::styled(t.to_string(), Style::default().fg(Color::Yellow))
    } else {
        Span::raw(token.to_string())
    }
}

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

fn item_matches(item: &str, filter: &str) -> bool {
    filter.is_empty() || item.to_lowercase().contains(&filter.to_lowercase())
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

    let tab_index = app.mode as usize;

    let tabs = Tabs::new(VIEW_TITLES.iter().copied())
        .select(tab_index)
        .divider(" | ") // Explicitly set divider to 3 chars
        .block(Block::default().borders(Borders::ALL).title("Vigil"))
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

    let filter_display = if app.filter_mode || !app.filter_input.is_empty() {
        format!(" [Filter: {}] ", app.filter_input)
    } else {
        String::new()
    };
    let status_text = format!(
        " [Mode: {:?}]{} [Press '?' for Help] [Press 'q' to Quit] ",
        app.mode, filter_display
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

fn dashboard_info_panel(f: &mut Frame, area: Rect, title: Line<'static>, lines: Vec<Line<'static>>, is_active: bool, is_collapsed: bool) {
    let block = Block::default().borders(Borders::ALL).title(title).border_style(get_border_style(is_active));
    if is_collapsed {
        f.render_widget(Paragraph::new("").block(block), area);
    } else {
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

pub fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let state = lock_state(&app.app_state);

    let mut row_constraints = vec![];
    
    let row1_collapsed = app.dashboard_collapsed[0] && app.dashboard_collapsed[1] && app.dashboard_collapsed[2];
    row_constraints.push(if row1_collapsed { Constraint::Length(3) } else { Constraint::Min(0) });
    
    let row2_collapsed = app.dashboard_collapsed[3] && app.dashboard_collapsed[4] && app.dashboard_collapsed[5];
    row_constraints.push(if row2_collapsed { Constraint::Length(3) } else { Constraint::Min(0) });
    
    let row3_collapsed = app.dashboard_collapsed[6];
    row_constraints.push(if row3_collapsed { Constraint::Length(3) } else { Constraint::Min(0) });

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    let row1 = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)]).split(rows[0]);
        
    let row2 = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)]).split(rows[1]);
        
    let row3 = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100)]).split(rows[2]);

    app.panel_areas = vec![
        row1[0], row1[1], row1[2],
        row2[0], row2[1], row2[2],
        row3[0],
    ];

    // 1. Kafka
    let (kafka_status, kafka_status_color) = if state.kafka_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    dashboard_info_panel(f, row1[0],
        Line::from(vec![Span::styled("Kafka ", Style::default().fg(Color::Cyan)), Span::raw("localhost:9092")]),
        vec![
            Line::from(vec![Span::raw("Status: "), Span::styled(kafka_status, Style::default().fg(kafka_status_color))]),
            Line::from(vec![Span::raw("Topics: "), Span::styled(state.kafka_topics.len().to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Msgs: "), Span::styled("N/A", Style::default().fg(Color::DarkGray))]),
        ],
        app.active_panel == 0,
        app.dashboard_collapsed[0],
    );

    // 2. Redis
    let (redis_status, redis_status_color) = if state.redis_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let total_keys: usize = state.redis_keys.values().map(|k| k.len()).sum();
    let redis_mem = state.redis_mem.clone();
    dashboard_info_panel(f, row1[1],
        Line::from(vec![Span::styled("Redis ", Style::default().fg(Color::Cyan)), Span::raw("localhost:6379")]),
        vec![
            Line::from(vec![Span::raw("Status: "), Span::styled(redis_status, Style::default().fg(redis_status_color))]),
            Line::from(vec![Span::raw("DBs: "), Span::styled(state.redis_dbs.to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Keys: "), Span::styled(total_keys.to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Memory: "), Span::styled(redis_mem, Style::default().fg(Color::Yellow))]),
        ],
        app.active_panel == 1,
        app.dashboard_collapsed[1],
    );

    // 3. RabbitMQ
    let (rabbitmq_status, rabbit_color) = if state.rabbitmq_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    dashboard_info_panel(f, row1[2],
        Line::from(vec![Span::styled("RabbitMQ ", Style::default().fg(Color::Cyan)), Span::raw("localhost:5672")]),
        vec![
            Line::from(vec![Span::raw("Status: "), Span::styled(rabbitmq_status, Style::default().fg(rabbit_color))]),
            Line::from(vec![Span::raw("Queues: "), Span::styled(state.rabbitmq_queues.len().to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Msgs: "), Span::styled("N/A", Style::default().fg(Color::DarkGray))]),
            Line::from(vec![Span::raw("Consumers: "), Span::styled("N/A", Style::default().fg(Color::DarkGray))]),
        ],
        app.active_panel == 2,
        app.dashboard_collapsed[2],
    );

    // 4. MongoDB
    let (mongo_status, mongo_color) = if state.mongo_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    let mongo_collections: usize = state.mongo_collections.values().map(|c| c.len()).sum();
    let mongo_size = state.mongo_db_size.clone();
    dashboard_info_panel(f, row2[0],
        Line::from(vec![Span::styled("MongoDB ", Style::default().fg(Color::Cyan)), Span::raw("localhost:27017")]),
        vec![
            Line::from(vec![Span::raw("Status: "), Span::styled(mongo_status, Style::default().fg(mongo_color))]),
            Line::from(vec![Span::raw("DBs: "), Span::styled(state.mongo_dbs.len().to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Collections: "), Span::styled(mongo_collections.to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Size: "), Span::styled(mongo_size, Style::default().fg(Color::Yellow))]),
        ],
        app.active_panel == 3,
        app.dashboard_collapsed[3],
    );

    // 5. InfluxDB
    let (influxdb_status, influxdb_color) = if state.influxdb_online { ("ONLINE", Color::Green) } else { ("OFFLINE", Color::Red) };
    dashboard_info_panel(f, row2[1],
        Line::from(vec![Span::styled("InfluxDB ", Style::default().fg(Color::Cyan)), Span::raw("localhost:8086")]),
        vec![
            Line::from(vec![Span::raw("Status: "), Span::styled(influxdb_status, Style::default().fg(influxdb_color))]),
            Line::from(vec![Span::raw("Orgs: "), Span::styled(state.influxdb_orgs.len().to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Buckets: "), Span::styled(state.influxdb_buckets.len().to_string(), Style::default().fg(Color::Yellow))]),
        ],
        app.active_panel == 4,
        app.dashboard_collapsed[4],
    );

    // 6. Redis Streams Overview
    let stream_count = state.redis_streams.len();
    let total_entries: usize = state.redis_stream_entries.values().map(|e| e.len()).sum();
    let streams_title = Line::from(vec![
        Span::styled("Redis Streams ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled(format!("({} streams, {} entries)", stream_count, total_entries), Style::default().fg(Color::DarkGray)),
    ]);

    let streams_block = Block::default()
        .borders(Borders::ALL)
        .title(streams_title)
        .border_style(get_border_style(app.active_panel == 5));

    if app.dashboard_collapsed[5] {
        f.render_widget(Paragraph::new("").block(streams_block), row2[2]);
    } else {
        let streams_inner = streams_block.inner(row2[2]);
        f.render_widget(streams_block, row2[2]);

        if state.redis_streams.is_empty() {
            let empty_msg = Paragraph::new(
                Line::from(vec![
                    Span::styled("No streams discovered. ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Streams will appear here when Redis is online.", Style::default().fg(Color::DarkGray)),
                ])
            );
            f.render_widget(empty_msg, streams_inner);
        } else {
            // Split the streams area into a summary header + stream list
            let streams_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(streams_inner);

            // Header row
            let header = Line::from(vec![
                Span::styled(" Stream Name", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("                              "),
                Span::styled("Entries", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("     "),
                Span::styled("Latest Entry", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]);
            f.render_widget(Paragraph::new(header), streams_layout[0]);

            // Stream rows
            let mut stream_lines: Vec<Line> = Vec::new();
            for stream_name in &state.redis_streams {
                let entries = state.redis_stream_entries.get(stream_name);
                let entry_count = entries.map_or(0, |e| e.len());
                let latest = entries
                    .and_then(|e| e.last())
                    .map(|e| {
                        if e.len() > 50 { format!("{}…", &e[..50]) } else { e.clone() }
                    })
                    .unwrap_or_else(|| "—".to_string());

                // Color-code entry count
                let count_color = if entry_count == 0 {
                    Color::DarkGray
                } else if entry_count < 100 {
                    Color::Green
                } else {
                    Color::Yellow
                };

                stream_lines.push(Line::from(vec![
                    Span::styled(" ▸ ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        format!("{:<30}", stream_name),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("{:<8}", entry_count),
                        Style::default().fg(count_color),
                    ),
                    Span::styled(latest, Style::default().fg(Color::DarkGray)),
                ]));
            }
            f.render_widget(Paragraph::new(stream_lines), streams_layout[1]);
        }
    }

    // 7. Logs
    let logs_text = if state.logs.is_empty() {
        "No logs yet. Logs will appear here...".to_string()
    } else {
        state.logs.join("\n")
    };
    let log_block = Block::default()
        .title(Span::styled("Logs", Style::default().fg(Color::Cyan)))
        .borders(Borders::ALL)
        .border_style(get_border_style(app.active_panel == 6));
    
    if app.dashboard_collapsed[6] {
        f.render_widget(Paragraph::new("").block(log_block), row3[0]);
    } else {
        f.render_widget(Paragraph::new(logs_text).block(log_block), row3[0]);
    }
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
        let r_state = lock_state(&app.app_state);
    let db_count = if r_state.redis_dbs > 0 { r_state.redis_dbs } else { 16 };
    let mut dbs = vec![];
    for i in 0..db_count {
        let is_sel = state[0] == i;
        let prefix = if is_sel { "▶" } else { " " };
        dbs.push(Line::from(vec![Span::styled(format!("{} DB {}", prefix, i), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(dbs).block(Block::default().title("Databases").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let filter_active = !app.filter_input.is_empty();
    let mut keys = vec![];
    let selected_db = state[0].min(db_count.saturating_sub(1));
    let db_keys: Vec<(String, String)> = if filter_active {
        r_state.redis_keys.get(&selected_db).map(|k| k.iter().filter(|(k, _)| item_matches(k, &app.filter_input)).cloned().collect()).unwrap_or_default()
    } else {
        r_state.redis_keys.get(&selected_db).cloned().unwrap_or_default()
    };
    
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
    let m_state = lock_state(&app.app_state);
    let filter_active = !app.filter_input.is_empty();
    let mut dbs: Vec<String> = if filter_active {
        m_state.mongo_dbs.iter().filter(|d| item_matches(d, &app.filter_input)).cloned().collect()
    } else {
        m_state.mongo_dbs.clone()
    };
    if dbs.is_empty() && !filter_active { dbs.push("Loading...".to_string()); }
    
    let mut db_lines = vec![];
    for (i, db) in dbs.iter().enumerate() {
        let is_sel = state[0] == i;
        let prefix = if is_sel { "▶" } else { " " };
        db_lines.push(Line::from(vec![Span::styled(format!("{} {}", prefix, db), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(db_lines).block(Block::default().title("Databases").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 0))), chunks[0]);

    let selected_db = state[0].min(dbs.len().saturating_sub(1));
    let db_name = dbs.get(selected_db).cloned().unwrap_or_default();
    
    let mut cols: Vec<String> = if filter_active {
        m_state.mongo_collections.get(&db_name).map(|c| c.iter().filter(|c| item_matches(c, &app.filter_input)).cloned().collect()).unwrap_or_default()
    } else {
        m_state.mongo_collections.get(&db_name).cloned().unwrap_or_default()
    };
    if cols.is_empty() && !filter_active { cols.push("None/Loading...".to_string()); }
    
    let mut col_lines = vec![];
    for (i, col) in cols.iter().enumerate() {
        let is_sel = state[1] == i;
        col_lines.push(Line::from(vec![Span::styled(col.clone(), item_style(is_sel))]));
    }
    f.render_widget(Paragraph::new(col_lines).block(Block::default().title("Collections").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 1))), chunks[1]);

    let selected_col = state[1].min(cols.len().saturating_sub(1));
    let col_name = cols.get(selected_col).cloned().unwrap_or_default();
    
    let mut docs: Vec<String> = if filter_active {
        m_state.mongo_docs.get(&(db_name, col_name)).map(|d| d.iter().filter(|d| item_matches(d, &app.filter_input)).cloned().collect()).unwrap_or_default()
    } else {
        m_state.mongo_docs.get(&(db_name, col_name)).cloned().unwrap_or_default()
    };
    if docs.is_empty() && !filter_active { docs.push("No documents found".to_string()); }
    
    let inner_w = chunks[2].width.saturating_sub(2);
    let separator = Line::from(Span::styled(
        format!(" {}", "─".repeat(inner_w.saturating_sub(1) as usize)),
        Style::default().fg(Color::DarkGray),
    ));
    let mut doc_lines = vec![];
    for (i, doc) in docs.iter().enumerate() {
        if i > 0 {
            doc_lines.push(separator.clone());
        }
        let is_sel = state[2] == i;
        // Compact multi-line JSON into a single-line preview
        let compact: String = doc.chars().filter(|&c| c != '\n' && c != '\r').collect();
        let display = if compact.len() > 120 {
            format!("{}…", &compact[..120])
        } else {
            compact
        };
        doc_lines.push(Line::from(vec![Span::styled(display, item_style(is_sel))]));
    }
    let mut doc_paragraph = Paragraph::new(doc_lines).block(Block::default().title("Documents").borders(Borders::ALL).border_style(get_border_style(app.active_panel == 2)));
    if app.wrap_enabled {
        doc_paragraph = doc_paragraph.wrap(Wrap { trim: false });
    }
    f.render_widget(doc_paragraph, chunks[2]);

    if let Some(ref doc_content) = app.expanded_doc {
        let popup_area = center(area, 80, 80);
        f.render_widget(Clear, popup_area);
        let block = Block::default()
            .title("Document (j/k scroll, w wrap, Esc close)")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);
        let doc_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner);
        let highlighted: Vec<Line> = doc_content
            .lines()
            .map(highlight_json_line)
            .collect();
        let mut popup = Paragraph::new(highlighted)
            .scroll((app.expanded_doc_scroll as u16, 0));
        if app.wrap_enabled {
            popup = popup.wrap(Wrap { trim: false });
        }
        f.render_widget(popup, doc_layout[0]);
        let wrap_status = if app.wrap_enabled { "ON" } else { "OFF" };
        f.render_widget(
            Paragraph::new(format!(" Wrap: {} | Scroll: {}", wrap_status, app.expanded_doc_scroll))
                .style(Style::default().fg(Color::DarkGray)),
            doc_layout[1],
        );
    }
}

pub fn draw_redis_streams(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);
    app.panel_areas = chunks.to_vec();
    let ui_state = app.states.redis_streams;
    let data = lock_state(&app.app_state);

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
    let data = lock_state(&app.app_state);

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
    let data = lock_state(&app.app_state);

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
