use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, TcpListener},
    sync::watch,
    time::{sleep, timeout, Duration, Interval, interval},
};

#[derive(Debug, Clone)]
struct SharedData {
    latest: String,
}

#[tokio::main]
async fn main() {
    let initial = SharedData {
        latest: "<no data>".to_string(),
    };
    let (tx, rx) = watch::channel(initial);

    let tcp_addr = "127.0.0.1:4000".to_string();
    let task_tx = tx.clone();
    tokio::spawn(async move {
        tcp_worker_loop(tcp_addr, task_tx).await;
    });

    let app_state = AppState {
        rx: Arc::new(rx),
    };
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/status", get(status_text_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("HTTP server listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    rx: Arc<watch::Receiver<SharedData>>,
}

async fn index_handler(State(state): State<AppState>) -> Html<String> {
    let data = (*state.rx.borrow()).clone();
    let html = render_html(&data);
    Html(html)
}

async fn status_text_handler(State(state): State<AppState>) -> impl IntoResponse {
    let data = (*state.rx.borrow()).clone();
    (
        [("content-type", "text/plain; charset=utf-8")],
        data.latest,
    )
}

fn render_html(data: &SharedData) -> String {
    // 簡易エスケープ（最小限）。必要なら html-escape クレートを使う。
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;")
         .replace('<', "&lt;")
         .replace('>', "&gt;")
         .replace('"', "&quot;")
         .replace('\'', "&#39;")
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Sample HTTP Server</title>
  <style>
    body {{ font-family: system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial; padding: 24px; }}
    pre {{ background:#f5f5f5; padding:12px; border-radius:6px; }}
    .meta {{ color:#666; font-size:0.9rem; }}
  </style>
  <script>
    // 1秒ごとに最新を取得して差し替える
    async function poll() {{
      try {{
        const res = await fetch('/status', {{ cache: 'no-store' }} );
        if (!res.ok) return;
        document.getElementById('latest').textContent = await res.text();
      }} catch (e) {{
        console.error(e);
      }}
    }}
    setInterval(poll, 1000);
    window.addEventListener('load', poll);
  </script>
</head>
<body>
  <h1>Sample HTTP Server</h1>
  <h2>Payload</h2>
  <pre id="latest">{latest}</pre>
</body>
</html>
"#,
        latest = esc(&data.latest),
    )
}

async fn tcp_worker_loop(addr: String, tx: watch::Sender<SharedData>) {
    loop {
        match TcpStream::connect(&addr).await {
            Ok(stream) => {
                println!("Connected to TCP server at {}", addr);
                if let Err(e) = handle_tcp_stream(stream, &tx).await {
                    eprintln!("TCP stream error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to connect to {}: {}. Retrying...", addr, e);
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn handle_tcp_stream(mut stream: TcpStream, tx: &watch::Sender<SharedData>) -> tokio::io::Result<()> {
    // split して書き込み側を使う
    let (r, mut w) = stream.split();
    let mut reader = BufReader::new(r);
    let mut line = String::new();

    // 20ms 周期
    let mut ticker: Interval = interval(Duration::from_millis(20));

    loop {
        // 次の tick を待つ（周期制御）
        ticker.tick().await;

        // 要求を送る
        if let Err(e) = w.write_all(b"GET\n").await {
            eprintln!("write failed: {}", e);
            return Err(e);
        }
        // 送信が TCP の内部バッファに入っただけでも先に進める -- flush を入れたければ追加
        if let Err(e) = w.flush().await {
            eprintln!("flush failed: {}", e);
            return Err(e);
        }

        // 読み取りはタイムアウト付きで行う（ここでは 50ms を上限にする例）
        line.clear();
        match timeout(Duration::from_millis(50), reader.read_line(&mut line)).await {
            Ok(Ok(n)) => {
                if n == 0 {
                    println!("Remote closed connection");
                    return Ok(());
                }
                let payload = line.trim_end().to_string();
                let new = SharedData {
                    latest: payload,
                };
                if tx.send(new).is_err() {
                    eprintln!("No receivers left for watch channel");
                }
            }
            Ok(Err(e)) => {
                // read error
                eprintln!("read_line error: {}", e);
                return Err(e);
            }
            Err(_) => {
                // timeout: 応答が来なかった -> 次の周期へ（何もしない）
                continue;
            }
        }
    }
}
