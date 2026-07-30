use std::sync::Mutex;

use tokio::sync::oneshot;

struct ServerState {
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl Drop for ServerState {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

#[tauri::command]
fn get_server_url(state: tauri::State<'_, Mutex<ServerState>>) -> String {
    state.lock().unwrap().url.clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");

    let (url, shutdown) = {
        let (tx, rx) = oneshot::channel::<()>();

        let (port, listener, app) = rt
            .block_on(async { aa_server::build(0, None, None, None).await })
            .expect("server build");

        let url = format!("http://localhost:{port}");
        eprintln!("[aa] In-process server on {url}");

        rt.spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async { rx.await.ok(); })
                .await
                .ok();
        });

        (url, Some(tx))
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(ServerState { url, shutdown }))
        .invoke_handler(tauri::generate_handler![get_server_url])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
