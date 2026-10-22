//! Independent literal ATX peer, deliberately ignoring all colour mutations.
use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot, Notify},
};

#[derive(Clone)]
pub struct Profile {
    pub device_type: u8,
    pub next: Vec<u8>,
    pub gear_features: Option<u8>,
    pub features: u8,
    pub control: u8,
    pub assignments: Vec<u8>,
    pub status: u8,
    pub high_failure: Option<&'static str>,
    pub low_failure: Option<&'static str>,
    pub hold: Option<u16>,
    pub pace: Duration,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            device_type: 8,
            next: vec![],
            gear_features: Some(0),
            features: 2,
            control: 0x80,
            assignments: vec![1, 2, 3],
            status: 0x20,
            high_failure: None,
            low_failure: None,
            hold: None,
            pace: Duration::ZERO,
        }
    }
}
pub struct Peer {
    pub path: PathBuf,
    pub profile: Arc<Mutex<Profile>>,
    pub commands: Arc<Mutex<Vec<u16>>>,
    pub held: Arc<Notify>,
    pub task: tokio::task::JoinHandle<()>,
    raw: mpsc::Sender<(Vec<u8>, oneshot::Sender<()>)>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    pub fn start(profile: Profile) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let profile = Arc::new(Mutex::new(profile));
        let state = profile.clone();
        let commands = Arc::new(Mutex::new(vec![]));
        let record = commands.clone();
        let held = Arc::new(Notify::new());
        let signal = held.clone();
        let (raw, mut receive) = mpsc::channel::<(Vec<u8>, oneshot::Sender<()>)>(4);
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = String::new();
            let mut dtr0 = 0;
            let mut low = 250;
            let mut next_index = 0;
            let mut level = 100;
            loop {
                tokio::select! {
                    raw = receive.recv() => match raw {
                        Some((bytes, reply)) => { write.write_all(&bytes).await.unwrap(); let _ = reply.send(()); },
                        None => return,
                    },
                    count = read.read_line(&mut line) => {
                        if count.unwrap() == 0 { return; }
                        let response: Option<String> = match line.as_str() {
                            "v\n" => Some("V013204\n".into()),
                            "d\n" => Some("D24\n".into()),
                            _ => {
                                assert!(line.starts_with('h') && line.len() == 6, "unexpected HAT line {line:?}");
                                let frame = u16::from_str_radix(&line[1..5],16).unwrap();
                                record.lock().unwrap().push(frame);
                                let p = state.lock().unwrap().clone();
                                if p.hold == Some(frame) { signal.notify_one(); None } else {
                                    let value = match frame {
                                        0x01a0 => Some(level), 0x0190 => Some(4),
                                        0x0199 => { next_index = 0; Some(p.device_type) },
                                        0x01a7 => { let v = p.next.get(next_index).copied().unwrap_or(254); next_index += 1; Some(v) },
                                        0x01f7 => p.gear_features,
                                        0x01f9 => Some(p.features), 0x01fb => Some(p.control),
                                        0x01fc => Some(p.assignments.get(dtr0 as usize).copied().unwrap_or(255)),
                                        0x01f8 => Some(p.status),
                                        0x01fa => {
                                            if let Some(raw) = p.high_failure { line.clear(); SomeRaw::send(&mut write, raw).await; continue; }
                                            let word: u16 = match dtr0 { 0=>4660,1=>9029,2=>250,128=>200,129=>153,130=>400,131=>500,9..=14=>7,_=>65535 };
                                            low = word as u8;
                                            Some(if (9..=14).contains(&dtr0) {7} else {(word>>8) as u8})
                                        },
                                        0x0198 => {
                                            if let Some(raw) = p.low_failure { line.clear(); SomeRaw::send(&mut write, raw).await; continue; }
                                            Some(low)
                                        },
                                        0x0100 => { level = 0; None },
                                        0xc108 => None,
                                        0x01e0..=0x01ec => None,
                                        raw if raw >> 8 == 0xa3 => { dtr0 = raw as u8; None },
                                        raw if matches!(raw >> 8, 0xc3 | 0xc5) => None,
                                        _ => panic!("unexpected independent colour-test request {frame:04x}"),
                                    };
                                    Some(value.map_or_else(|| "N\n".into(), |raw| format!("J{raw:02X}\n")))
                                }
                            }
                        };
                        line.clear();
                        if let Some(bytes) = response {
                            let pace = state.lock().unwrap().pace;
                            tokio::time::sleep(pace).await;
                            write.write_all(bytes.as_bytes()).await.unwrap();
                        }
                    }
                }
            }
        });
        Self {
            path,
            profile,
            commands,
            held,
            task,
            raw,
        }
    }
    pub async fn send(&self, bytes: &[u8]) {
        let (send, receive) = oneshot::channel();
        self.raw.try_send((bytes.to_vec(), send)).unwrap();
        tokio::time::timeout(Duration::from_secs(1), receive)
            .await
            .unwrap()
            .unwrap();
    }
    pub fn setters(&self) -> Vec<u16> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .copied()
            .filter(|v| (0x01e0..=0x01ec).contains(v))
            .collect()
    }
}
struct SomeRaw;
impl SomeRaw {
    async fn send<W: AsyncWriteExt + Unpin>(write: &mut W, raw: &str) {
        write.write_all(raw.as_bytes()).await.unwrap();
    }
}
