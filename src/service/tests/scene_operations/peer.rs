//! Literal ATX peer: stored report selectors and bytes are independent of codecs.
use super::*;
use std::{collections::BTreeMap, sync::Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[derive(Clone)]
pub(super) struct Profile {
    pub device_type: &'static str,
    pub kind: u8,
    pub values: BTreeMap<u8, u16>,
    pub high_failure: Option<(u8, &'static str)>,
    pub low_failure: Option<(u8, &'static str)>,
    pub recall: &'static str,
    pub hold_recall: bool,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            device_type: "J08\n",
            kind: 0x20,
            values: BTreeMap::from([(226, 350)]),
            high_failure: None,
            low_failure: None,
            recall: "N\n",
            hold_recall: false,
        }
    }
}
pub(super) struct Peer {
    pub path: PathBuf,
    pub frames: Arc<Mutex<Vec<u16>>>,
    pub held: Arc<tokio::sync::Notify>,
    pub release: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    pub fn new(profile: Profile) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let record = frames.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (signal, resume) = (held.clone(), release.clone());
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = String::new();
            let (mut dtr, mut low, mut selected) = (0u8, 0u8, false);
            loop {
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let response = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        assert!(
                            line.starts_with('h') && line.len() == 6,
                            "unexpected scene peer command {line:?}"
                        );
                        let frame = u16::from_str_radix(&line[1..5], 16).unwrap();
                        record.lock().unwrap().push(frame);
                        let value = match frame {
                            0x0199 => {
                                write
                                    .write_all(profile.device_type.as_bytes())
                                    .await
                                    .unwrap();
                                line.clear();
                                continue;
                            }
                            0x01f7 => Some(0),
                            0x01f9 => Some(0xc3),
                            0x01fb => Some(0x80),
                            0x01fc => Some(dtr + 1),
                            0x01f8 => Some(0x20),
                            0x01a0 => {
                                selected = false;
                                Some(100)
                            }
                            0x0190 => Some(4),
                            0x01b0..=0x01bf => {
                                selected = true;
                                Some(255)
                            }
                            0x01fa => {
                                if selected {
                                    if let Some((selector, raw)) = profile.high_failure {
                                        if selector == dtr {
                                            write.write_all(raw.as_bytes()).await.unwrap();
                                            line.clear();
                                            continue;
                                        }
                                    }
                                }
                                let word = if selected {
                                    if dtr == 240 {
                                        u16::from(profile.kind)
                                    } else {
                                        *profile.values.get(&dtr).unwrap_or(&65535)
                                    }
                                } else {
                                    match dtr {
                                        2 => 250,
                                        128 => 200,
                                        129 => 153,
                                        130 => 400,
                                        131 => 500,
                                        _ => 65535,
                                    }
                                };
                                low = word as u8;
                                Some(if selected && (233..=240).contains(&dtr) {
                                    word as u8
                                } else {
                                    (word >> 8) as u8
                                })
                            }
                            0x0198 => {
                                if selected {
                                    if let Some((selector, raw)) = profile.low_failure {
                                        if selector == dtr {
                                            write.write_all(raw.as_bytes()).await.unwrap();
                                            line.clear();
                                            continue;
                                        }
                                    }
                                }
                                Some(low)
                            }
                            0xff10 | 0x8110 => {
                                if profile.hold_recall {
                                    write.write_all(b"L008001\n").await.unwrap();
                                    signal.notify_one();
                                    resume.notified().await;
                                }
                                write.write_all(profile.recall.as_bytes()).await.unwrap();
                                line.clear();
                                continue;
                            }
                            0xc108 => None,
                            f if f >> 8 == 0xa3 => {
                                dtr = f as u8;
                                None
                            }
                            _ => panic!(
                                "scene inspection issued a mutation/unexpected frame {frame:04x}"
                            ),
                        };
                        value.map_or_else(|| "N\n".into(), |v| format!("J{v:02X}\n"))
                    }
                };
                write.write_all(response.as_bytes()).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            frames,
            held,
            release,
            task,
        }
    }
}
