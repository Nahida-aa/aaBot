use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph, Wrap};

use super::app::App;
use super::types::MsgKind;

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::new(Direction::Vertical, [
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(area);
        let [msg_area, input_area] = [chunks[0], chunks[1]];
        self.render_messages(msg_area, frame);
        self.render_toast(msg_area, frame);
        self.render_input(input_area, frame);
    }

    fn render_messages(&mut self, area: Rect, frame: &mut Frame) {
        self.msg_line_map.clear();
        self.msg_area_y = area.y;
        self.msg_area_height = area.height;

        let sel_range = self.selected_range();
        let dim = Style::new().dim();
        let mut all_lines: Vec<Line> = Vec::new();

        for (msg_idx, msg) in self.messages.iter().enumerate() {
            let is_sel = sel_range.as_ref().map_or(false, |&(s, e)| msg_idx >= s && msg_idx <= e);
            let msg_lines = match &msg.kind {
                MsgKind::User => {
                    let mut lines = vec![Line::from(vec![
                        Span::styled("┃ ", Style::new().green()),
                        Span::styled("You", Style::new().green().bold()),
                    ])];
                    for line in msg.content.lines() {
                        lines.push(Line::from(Span::raw(line.to_string())));
                    }
                    lines.push(Line::from(Span::raw("")));
                    lines
                }
                MsgKind::Assistant => {
                    let mut lines = vec![Line::from(vec![
                        Span::styled("┃ ", Style::new().cyan()),
                        Span::styled("AI", Style::new().cyan().bold()),
                    ])];
                    lines.extend(super::super::markdown::render(&msg.content));
                    lines
                }
                MsgKind::ToolResult { content, is_error } => {
                    let color = if *is_error { Color::Red } else { Color::DarkGray };
                    content.lines().map(|line| {
                        Line::from(vec![
                            Span::styled("  └─ ", Style::new().fg(color)),
                            Span::styled(line.to_string(), Style::new().fg(color)),
                        ])
                    }).collect()
                }
                MsgKind::Error(e) => {
                    vec![
                        Line::from(vec![
                            Span::styled("⚠ ", Style::new().red().bold()),
                            Span::styled(e, Style::new().red()),
                        ]),
                        Line::from(Span::raw("")),
                    ]
                }
            };

            for mut line in msg_lines {
                if is_sel {
                    for span in &mut line.spans {
                        span.style = span.style.bg(Color::DarkGray);
                    }
                }
                all_lines.push(line);
                self.msg_line_map.push(msg_idx);
            }
        }

        if let Some(ref text) = self.streaming_text {
            all_lines.push(Line::from(vec![
                Span::styled("┃ ", Style::new().cyan()),
                Span::styled("AI", Style::new().cyan().bold()),
            ]));
            if text.is_empty() {
                all_lines.push(Line::from(Span::styled("▊", dim)));
            } else {
                all_lines.extend(super::super::markdown::render(text));
            }
        }

        if let Some(ref name) = self.active_tool_name {
            all_lines.push(Line::from(vec![
                Span::styled("  ↻ ", Style::new().yellow()),
                Span::styled(name.clone(), Style::new().yellow().bold()),
                Span::styled(" running...", dim),
            ]));
        }

        if all_lines.is_empty() {
            all_lines.push(Line::from(Span::styled(
                "Type a message and press Enter to start.",
                dim,
            )));
            all_lines.push(Line::from(Span::raw("")));
        }

        let line_count = all_lines.len() as u16;
        let viewport_height = area.height.max(1);
        let max_scroll = line_count.saturating_sub(viewport_height);
        if self.auto_scroll {
            self.scroll = max_scroll;
        }
        let scroll = self.scroll.min(max_scroll);

        if scroll > 0 {
            all_lines.insert(0, Line::from(Span::styled(
                format!("↑ {} more lines", scroll), dim,
            )));
        }

        let paragraph = Paragraph::new(all_lines)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn render_input(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::new(Direction::Vertical, [
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
        let [input_line, status_line, _spacer] = [chunks[0], chunks[1], chunks[2]];

        let prompt = format!("> {}", self.input);
        frame.render_widget(Paragraph::new(prompt.as_str()), input_line);

        let cursor_x = (2 + self.input_cursor) as u16;
        frame.set_cursor_position((input_line.x + cursor_x, input_line.y));

        let status = format!(
            " {} | {} tools | {} tokens | Ctrl+Y copy msg | Esc exit",
            self.model_name, self.tools_count, self.token_count,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(status, Style::new().dim())),
            status_line,
        );
    }

    fn render_toast(&self, area: Rect, frame: &mut Frame) {
        let Some(ref toast) = self.toast else { return };
        let width = (toast.len() + 6).min(area.width.saturating_sub(2) as usize) as u16;
        let height = 3;
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height / 4).max(1);
        let toast_area = Rect::new(x, y.min(area.height.saturating_sub(height)), width, height);

        frame.render_widget(Clear, toast_area);
        let block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::new().fg(Color::Green));
        let text = Span::styled(
            format!(" {} ", toast),
            Style::new().fg(Color::Green).bold(),
        );
        let inner = block.inner(toast_area);
        frame.render_widget(block, toast_area);
        frame.render_widget(Paragraph::new(text).centered(), inner);
    }
}
