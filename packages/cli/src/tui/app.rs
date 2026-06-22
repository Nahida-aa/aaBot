use std::collections::VecDeque;
use std::sync::mpsc;

use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};

use super::types::*;

pub struct App {
    pub messages: Vec<DisplayMessage>,
    pub streaming_text: Option<String>,
    pub tool_calls_in_flight: Vec<String>,

    pub input: String,
    pub input_cursor: usize,
    pub history: VecDeque<String>,
    history_pos: usize,

    pub scroll: u16,
    pub auto_scroll: bool,

    pub tools_count: usize,
    pub model_name: String,
    pub token_count: u32,
    pub active_tool_name: Option<String>,

    pub toast: Option<String>,
    pub toast_ticks: u8,

    pub selection: Option<(usize, usize)>,
    pub mouse_selecting: bool,
    pub msg_line_map: Vec<usize>,
    pub msg_area_y: u16,
    pub msg_area_height: u16,

    pub exit: bool,
    pub ui_rx: mpsc::Receiver<UiEvent>,
    pub worker_tx: mpsc::Sender<WorkerCommand>,
}

impl App {
    pub fn new(ui_rx: mpsc::Receiver<UiEvent>, worker_tx: mpsc::Sender<WorkerCommand>) -> Self {
        Self {
            messages: Vec::new(),
            streaming_text: None,
            tool_calls_in_flight: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            history: VecDeque::new(),
            history_pos: 0,
            scroll: 0,
            auto_scroll: true,
            tools_count: 0,
            model_name: String::new(),
            token_count: 0,
            active_tool_name: None,
            toast: None,
            toast_ticks: 0,
            selection: None,
            mouse_selecting: false,
            msg_line_map: Vec::new(),
            msg_area_y: 0,
            msg_area_height: 0,
            exit: false,
            ui_rx,
            worker_tx,
        }
    }

    pub fn show_toast(&mut self, msg: String) {
        // 直接写 stderr 确保可见
        eprintln!("\n  ✓ {}  \n", msg);
        // 同时保留 Ratatui 浮动 overlay
        self.toast = Some(msg);
        self.toast_ticks = 10;
    }

    pub fn tick_toast(&mut self) {
        if self.toast_ticks > 0 {
            self.toast_ticks -= 1;
            if self.toast_ticks == 0 {
                self.toast = None;
            }
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        tracing::info!("key: {:?} mod={:?}", key.code, key.modifiers);

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    tracing::info!("Ctrl+C");
                    if self.selection.is_some() {
                        self.copy_selected_messages();
                    } else {
                        self.copy_all_messages();
                    }
                    return;
                }
                KeyCode::Char('y') => { tracing::info!("Ctrl+Y -> copy_last_assistant"); self.copy_last_assistant(); return; }
                KeyCode::Char('v') => {
                    if let Some(text) = crate::clipboard::read() {
                        self.input.insert_str(self.input_cursor, &text);
                        self.input_cursor += text.chars().count();
                    }
                    return;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Esc => {
                if self.selection.is_some() {
                    self.selection = None;
                    return;
                }
                self.exit = true;
            }
            KeyCode::Enter => {
                let input = self.input.trim().to_owned();
                if input.is_empty() { return; }
                if self.history.is_empty() || self.history.back().map_or(true, |h| *h != input) {
                    self.history.push_back(input.clone());
                }
                self.history_pos = self.history.len();
                self.messages.push(DisplayMessage {
                    kind: MsgKind::User,
                    content: input.clone(),
                });
                self.input.clear();
                self.input_cursor = 0;
                self.auto_scroll = true;
                self.streaming_text = Some(String::new());
                self.tool_calls_in_flight.clear();
                self.active_tool_name = None;
                let _ = self.worker_tx.send(WorkerCommand::Submit {
                    messages: vec![aa_core::llm::Message {
                        role: aa_core::llm::Role::User,
                        content: input,
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    }],
                });
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input.remove(self.input_cursor - 1);
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Left => { if self.input_cursor > 0 { self.input_cursor -= 1; } }
            KeyCode::Right => { if self.input_cursor < self.input.len() { self.input_cursor += 1; } }
            KeyCode::Up => {
                if self.history_pos > 0 {
                    self.history_pos -= 1;
                    self.input = self.history[self.history_pos].clone();
                    self.input_cursor = self.input.len();
                }
            }
            KeyCode::Down => {
                if self.history_pos + 1 < self.history.len() {
                    self.history_pos += 1;
                    self.input = self.history[self.history_pos].clone();
                    self.input_cursor = self.input.len();
                } else {
                    self.history_pos = self.history.len();
                    self.input.clear();
                    self.input_cursor = 0;
                }
            }
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            KeyCode::PageUp => { self.scroll = self.scroll.saturating_add(5); self.auto_scroll = false; }
            KeyCode::PageDown => { self.scroll = self.scroll.saturating_sub(5); }
            KeyCode::Char(c) => { self.input.insert(self.input_cursor, c); self.input_cursor += 1; }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind == MouseEventKind::Moved { return; }

        let y_in_area = (mouse.row as i16) - (self.msg_area_y as i16);
        let line_idx = (y_in_area + (self.scroll as i16)) as usize;
        let in_msg_area = y_in_area >= 0 && line_idx < self.msg_line_map.len();

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !in_msg_area {
                    self.selection = None;
                    self.mouse_selecting = false;
                    return;
                }
                let msg_idx = self.msg_line_map[line_idx];
                self.selection = Some((msg_idx, msg_idx));
                self.mouse_selecting = true;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if !self.mouse_selecting { return; }
                if !in_msg_area {
                    if y_in_area < 0 {
                        // dragged above message area, select first message
                        self.selection = self.selection.map(|(a, _)| (a, 0));
                    }
                    return;
                }
                let msg_idx = self.msg_line_map[line_idx];
                self.selection = self.selection.map(|(a, _)| (a, msg_idx));
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.mouse_selecting && self.selection.is_some() {
                    self.mouse_selecting = false;
                    self.copy_selected_messages();
                }
            }
            _ => {}
        }
    }

    pub fn handle_event(&mut self, ev: UiEvent) {
        match ev {
            UiEvent::Ready { tools, model } => { self.tools_count = tools; self.model_name = model; }
            UiEvent::Token(text) => {
                if let Some(ref mut buf) = self.streaming_text { buf.push_str(&text); }
                self.active_tool_name = None;
            }
            UiEvent::ToolCall(tc) => {
                let name = tc.function.name.clone();
                self.tool_calls_in_flight.push(name.clone());
                self.active_tool_name = Some(name);
            }
            UiEvent::ToolResult { name, content, is_error } => {
                if let Some(pos) = self.tool_calls_in_flight.iter().position(|n| *n == name) {
                    self.tool_calls_in_flight.remove(pos);
                }
                self.messages.push(DisplayMessage {
                    kind: MsgKind::ToolResult { content, is_error },
                    content: String::new(),
                });
                self.active_tool_name = None;
            }
            UiEvent::Done { usage } => {
                let text = self.streaming_text.take().unwrap_or_default();
                self.messages.push(DisplayMessage {
                    kind: MsgKind::Assistant,
                    content: text,
                });
                if let Some(u) = usage { self.token_count = u.total_tokens; }
                self.tool_calls_in_flight.clear();
                self.active_tool_name = None;
            }
            UiEvent::Error(msg) => {
                self.streaming_text = None;
                self.messages.push(DisplayMessage {
                    kind: MsgKind::Error(msg),
                    content: String::new(),
                });
                self.tool_calls_in_flight.clear();
                self.active_tool_name = None;
            }
        }
    }

    pub fn selected_range(&self) -> Option<(usize, usize)> {
        self.selection.map(|(a, c)| (a.min(c), a.max(c)))
    }

    fn copy_selected_messages(&mut self) {
        let range = match self.selected_range() {
            Some(r) => r,
            None => return,
        };
        let mut text = String::new();
        for (i, msg) in self.messages.iter().enumerate() {
            if i < range.0 || i > range.1 { continue; }
            match &msg.kind {
                MsgKind::User => { text.push_str("You:\n"); text.push_str(&msg.content); text.push('\n'); }
                MsgKind::Assistant => { text.push_str("AI:\n"); text.push_str(&msg.content); text.push('\n'); }
                MsgKind::ToolResult { content, is_error } => {
                    let label = if *is_error { "Error" } else { "Tool" };
                    text.push_str(&format!("{}:\n{}\n", label, content));
                }
                MsgKind::Error(e) => { text.push_str(&format!("Error:\n{}\n", e)); }
            }
        }
        self.selection = None;
        if !text.is_empty() { crate::clipboard::write(&text); self.show_toast("Copied!".into()); }
    }

    fn copy_all_messages(&mut self) {
        tracing::info!("copy_all_messages called");
        let mut text = String::new();
        for msg in &self.messages {
            match &msg.kind {
                MsgKind::User => { text.push_str("You:\n"); text.push_str(&msg.content); text.push('\n'); }
                MsgKind::Assistant => { text.push_str("AI:\n"); text.push_str(&msg.content); text.push('\n'); }
                _ => {}
            }
        }
        if let Some(ref t) = self.streaming_text {
            if !t.is_empty() { text.push_str("AI:\n"); text.push_str(t); text.push('\n'); }
        }
        if !text.is_empty() { crate::clipboard::write(&text); self.show_toast("Copied!".into()); }
    }

    fn copy_last_assistant(&mut self) {
        tracing::info!("copy_last_assistant called");
        let found = self.messages.iter().rev().find_map(|msg| {
            if matches!(msg.kind, MsgKind::Assistant) { Some(msg.content.clone()) } else { None }
        });
        match found {
            Some(text) => { crate::clipboard::write(&text); self.show_toast("Copied!".into()); }
            None => {
                if let Some(ref t) = self.streaming_text {
                    if !t.is_empty() { crate::clipboard::write(t); self.show_toast("Copied!".into()); }
                }
            }
        }
    }
}
