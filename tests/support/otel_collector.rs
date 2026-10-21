//! Bounded local HTTP OTLP sink used only by integration tests.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
type Record = (String, Vec<u8>);
pub struct Collector {
    pub endpoint: String,
    records: Arc<Mutex<Vec<Record>>>,
    task: JoinHandle<()>,
}
impl Collector {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let records = Arc::new(Mutex::new(Vec::new()));
        let saved = records.clone();
        let task = tokio::spawn(async move {
            for _ in 0..64 {
                let (stream, _) = listener.accept().await.unwrap();
                if let Ok(Some(record)) =
                    tokio::time::timeout(Duration::from_secs(2), receive(stream)).await
                {
                    saved.lock().unwrap().push(record);
                }
            }
        });
        Self {
            endpoint,
            records,
            task,
        }
    }
    pub fn has_operation(&self, trace: &[u8; 16], operation: &str) -> bool {
        let expected: String = trace.iter().map(|byte| format!("{byte:02x}")).collect();
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|(path, _)| path == "/v1/traces")
            .filter_map(|(_, body)| serde_json::from_slice::<serde_json::Value>(body).ok())
            .any(|request| {
                request["resourceSpans"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|resource| resource["scopeSpans"].as_array().into_iter().flatten())
                    .flat_map(|scope| scope["spans"].as_array().into_iter().flatten())
                    .any(|span| {
                        span["traceId"].as_str() == Some(expected.as_str())
                            && span["name"] == "bridge_operation"
                            && span["attributes"].as_array().into_iter().flatten().any(
                                |attribute| {
                                    attribute["key"] == "operation"
                                        && attribute["value"]["stringValue"].as_str()
                                            == Some(operation)
                                },
                            )
                    })
            })
    }
    pub fn summary(&self) -> Vec<(String, usize, String)> {
        self.records
            .lock()
            .unwrap()
            .iter()
            .map(|(path, body)| {
                (
                    path.clone(),
                    body.len(),
                    String::from_utf8_lossy(&body[..body.len().min(80)]).into_owned(),
                )
            })
            .collect()
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn receive(mut stream: TcpStream) -> Option<(String, Vec<u8>)> {
    let mut header = Vec::new();
    loop {
        header.push(stream.read_u8().await.ok()?);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        if header.len() > 8192 {
            return None;
        }
    }
    let header = String::from_utf8(header).ok()?;
    let path = header.lines().next()?.split_whitespace().nth(1)?.to_owned();
    let length = header
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    if length > 1024 * 1024 {
        return None;
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.ok()?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.ok()?;
    Some((path, body))
}
