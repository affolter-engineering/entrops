//! Leptos web app streaming entropy data from all USB serial devices via SSE.

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html,
    },
    routing::get,
    Json, Router,
};
use futures::stream::{self, Stream};
use leptos::prelude::*;
use num_bigint::BigUint;
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

/// Shared application state containing the list of devices and a broadcast channel for samples.
struct AppState {
    devices: Vec<String>,
    tx: broadcast::Sender<Sample>,
}

/// Representation of a sample read from a USB serial device.
#[derive(Clone, Serialize)]
struct Sample {
    device: String,
    timestamp: u64,
    hex: String,
    int: String,
    raw: String,
}

/// Read a sample from the specified USB serial device.
fn read_sample(device: &str) -> Result<Sample, String> {
    let port = serialport::new(device, 921600)
        .timeout(Duration::from_secs(10))
        .open()
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    BufReader::new(port)
        .read_until(b'\n', &mut data)
        .map_err(|e| e.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Ok(Sample {
        device: device.to_string(),
        timestamp,
        hex: hex::encode(&data),
        int: BigUint::from_bytes_be(&data).to_string(),
        raw: format!("{:?}", data),
    })
}

/// Leptos component for the main page of the web app.
#[component]
fn App(devices: Vec<String>) -> impl IntoView {
    let entries: Vec<(String, String)> = devices
        .iter()
        .map(|d| (d.clone(), d.replace('/', "-")))
        .collect();

    view! {
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <title>"entrops"</title>
                <script src="https://cdn.tailwindcss.com"></script>
            </head>
            <body class="font-mono bg-gray-50 h-screen flex flex-col overflow-hidden">
                <header class="flex items-center justify-between px-8 py-4 bg-white border-b border-gray-200 shrink-0">
                    <h1 class="text-xl font-bold text-gray-800">"entrops " <span class="text-gray-500 text-sm">"Entropy from a PRG320"</span></h1>
                    <button id="toggle"
                        class="px-4 py-2 text-sm font-medium text-white bg-red-500 rounded-lg hover:bg-re dataed-600">
                        "Stop"
                    </button>
                </header>
                <main class="flex-1 flex flex-col px-8 py-6 overflow-hidden">
                    {if entries.is_empty() {
                        view! {
                            <p class="text-red-500 text-sm">"No USB serial devices found."</p>
                        }.into_any()
                    } else {
                        view! {
                            <div class="flex gap-4 flex-1 min-h-0">
                                {entries.into_iter().map(|(device, id)| view! {
                                    <div class="flex-1 min-w-0 flex flex-col">
                                        <div class="flex items-center justify-between mb-1 shrink-0">
                                            <span class="text-xs font-semibold text-gray-500">Device: {device}</span>
                                            <span id=format!("{id}-ts") class="ts text-xs text-gray-400">
                                                "connecting\u{2026}"
                                            </span>
                                        </div>
                                        <div class="border border-gray-200 rounded-xl p-3 flex-1 overflow-hidden">
                                            <code id=format!("{id}-hex")
                                                class="block h-full text-xs text-gray-700 break-all overflow-hidden leading-relaxed">
                                                "\u{2014}"
                                            </code>
                                        </div>
                                    </div>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </main>
                <footer class="shrink-0 px-8 py-3 bg-white border-t border-gray-200 text-xs text-gray-400">
                    "© affolter engineering · v" {env!("CARGO_PKG_VERSION")}
                </footer>
                <script>
                    {r"var es=null;
var btn=document.getElementById('toggle');
function onMessage(ev){
  var d=JSON.parse(ev.data),id=d.device.split('/').join('-');
  var ts=document.getElementById(id+'-ts');
  if(d.error){if(ts)ts.textContent='error: '+d.error;}
  else{
    var h=document.getElementById(id+'-hex');
    if(h)h.textContent=d.hex;
    if(ts)ts.textContent=new Date().toLocaleTimeString();
  }
}
function onError(){document.querySelectorAll('.ts').forEach(function(e){e.textContent='disconnected';});}
function startStream(){
  es=new EventSource('/stream');
  es.onmessage=onMessage;
  es.onerror=onError;
  btn.textContent='Stop';
  btn.className='px-4 py-2 text-sm font-medium text-white bg-red-500 rounded-lg hover:bg-red-600';
}
function stopStream(){
  if(es){es.close();es=null;}
  document.querySelectorAll('.ts').forEach(function(e){e.textContent='stopped';});
  btn.textContent='Start';
  btn.className='px-4 py-2 text-sm font-medium text-white bg-green-500 rounded-lg hover:bg-green-600';
}
btn.onclick=function(){if(es){stopStream();}else{startStream();}};
startStream();"}
                </script>
            </body>
        </html>
    }
}

/// Render the main page of the web app.
async fn page(State(state): State<Arc<AppState>>) -> Html<String> {
    let devices = state.devices.clone();
    let owner = Owner::new();
    let html = owner.with(|| view! { <App devices=devices/> }.to_html());
    Html(format!("<!DOCTYPE html>{html}"))
}

/// Handle the /stream endpoint, returning samples as SSE events.
async fn sse(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send> {
    let rx = state.tx.subscribe();
    let stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(s) => {
                    let json = serde_json::json!({"device": s.device, "hex": s.hex, "int": s.int});
                    return Some((Ok(Event::default().data(json.to_string())), rx));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Collect samples from all devices, waiting up to 15 seconds for each.
async fn collect_samples(state: &Arc<AppState>) -> Vec<Sample> {
    let mut rx = state.tx.subscribe();
    let mut results: HashMap<String, Sample> = HashMap::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while results.len() < state.devices.len() {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(s)) => { results.entry(s.device.clone()).or_insert(s); }
            _ => break,
        }
    }
    state.devices.iter().filter_map(|d| results.remove(d)).collect()
}

/// Handle the /api endpoint, returning all samples as JSON.
async fn api(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    Json(collect_samples(&state).await.into_iter()
        .map(|s| serde_json::json!({"timestamp": s.timestamp, "hex": s.hex, "int": s.int, "raw": s.raw})).collect())
}

/// Return only the hex values of the samples.
async fn api_hex(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    Json(collect_samples(&state).await.into_iter()
        .map(|s| serde_json::json!({"timestamp": s.timestamp, "hex": s.hex})).collect())
}

/// Handle the /api/int endpoint, returning the integer representation of the samples.
async fn api_int(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    Json(collect_samples(&state).await.into_iter()
        .map(|s| serde_json::json!({"timestamp": s.timestamp, "int": s.int})).collect())
}

/// Return the raw data from all devices as JSON.
async fn api_raw(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    Json(collect_samples(&state).await.into_iter()
        .map(|s| serde_json::json!({"timestamp": s.timestamp, "raw": s.raw})).collect())
}

/// Handle a TCP client connection and stream samples to it.
async fn handle_tcp_client(mut socket: TcpStream, state: Arc<AppState>) {
    use tokio::io::AsyncWriteExt;
    let mut rx = state.tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(sample) => {
                if socket
                    .write_all(format!("{}\n", sample.hex).as_bytes())
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

///
async fn tcp_server(state: Arc<AppState>) {
    let listener = TcpListener::bind("0.0.0.0:3004").await.unwrap();
    println!("TCP streaming on 0.0.0.0:3004");
    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let state = state.clone();
                tokio::spawn(async move { handle_tcp_client(socket, state).await });
            }
            Err(e) => eprintln!("TCP accept error: {e}"),
        }
    }
}

/// Detect all USB serial devices connected to the system.
fn detect_devices() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| matches!(p.port_type, serialport::SerialPortType::UsbPort(_)))
        .map(|p| p.port_name)
        .collect()
}

/// Main entry point for the application.
#[tokio::main]
async fn main() {
    let devices = detect_devices();
    if devices.is_empty() {
        eprintln!("Warning: no USB serial devices detected");
    } else {
        println!("Devices: {}", devices.join(", "));
    }

    let (tx, _) = broadcast::channel::<Sample>(64);
    for device in &devices {
        let tx2 = tx.clone();
        let device = device.clone();
        tokio::spawn(async move {
            loop {
                let d = device.clone();
                let result = tokio::task::spawn_blocking(move || read_sample(&d))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                match result {
                    Ok(sample) => {
                        let _ = tx2.send(sample);
                    }
                    Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
                }
            }
        });
    }

    let state = Arc::new(AppState { devices, tx });
    tokio::spawn(tcp_server(state.clone()));

    let app = Router::new()
        .route("/", get(page))
        .route("/stream", get(sse))
        .route("/api", get(api))
        .route("/api/hex", get(api_hex))
        .route("/api/int", get(api_int))
        .route("/api/raw", get(api_raw))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3003").await.unwrap();
    println!("Listening on http://0.0.0.0:3003");
    axum::serve(listener, app).await.unwrap();
}
