mod app;
mod render;
pub(crate) mod types;
mod worker;

use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind, EnableBracketedPaste, DisableBracketedPaste, EnableMouseCapture, DisableMouseCapture};

use app::App;

pub fn run_tui(provider: Option<&str>, model: Option<&str>, base_url: Option<&str>, working_dir: &str) {
    let (ui_tx, ui_rx) = mpsc::channel();
    let (worker_tx, worker_rx) = mpsc::channel();

    let wp = working_dir.to_owned();
    let pr = provider.map(String::from);
    let mo = model.map(String::from);
    let bu = base_url.map(String::from);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio rt");
        if let Err(e) = rt.block_on(worker::worker_main(
            worker_rx, ui_tx, pr.as_deref(), mo.as_deref(), bu.as_deref(), &wp,
        )) {
            tracing::error!("worker failed: {e}");
        }
    });

    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste, EnableMouseCapture);
    let mut app = App::new(ui_rx, worker_tx);

    while !app.exit {
        let _ = terminal.draw(|f| app.render(f));
        app.tick_toast();

        if let Ok(true) = event::poll(Duration::from_millis(50)) {
            if let Ok(event) = event::read() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => { app.handle_key(key); }

                    Event::Mouse(mouse) => { app.handle_mouse(mouse); }

                    Event::Paste(text) => {
                        app.input.insert_str(app.input_cursor, &text);
                        app.input_cursor += text.chars().count();
                    }
                    _ => {}
                }
            }
        }

        while let Ok(ev) = app.ui_rx.try_recv() {
            app.handle_event(ev);
        }
    }

    let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste, DisableMouseCapture);
    ratatui::restore();
}
