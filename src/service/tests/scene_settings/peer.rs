//! Literal ATX peer with an existing pending colour, independent of DALI codecs.
use super::*;
use std::sync::{atomic::AtomicU8, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub(super) struct Peer {
    pub path: PathBuf,
    pub level: Arc<AtomicU8>,
    pub kind: Arc<AtomicU8>,
    pub frames: Arc<Mutex<Vec<String>>>,
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
    // Fault: 1 Remove has no effect; 2 Store has no effect; 3 hold Store terminal;
    // 4 Store collision. Initial stored Tc350, pending Tc777, actual Tc250.
    pub fn new(fault: u8) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let level = Arc::new(AtomicU8::new(42));
        let current = level.clone();
        let kind = Arc::new(AtomicU8::new(0x20));
        let stored_type = kind.clone();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let record = frames.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let signal = held.clone();
        let release = Arc::new(tokio::sync::Notify::new());
        let resume = release.clone();
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = String::new();
            let (mut dtr, mut dtr1, mut low, mut selected) = (0u8, 0u8, 0u8, false);
            let (mut temp_type, mut temp_tc, mut stored_tc) = (0x20u8, 777u16, 350u16);
            loop {
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let response = match line.as_str() {
                    "v\n" => "V013204\n".into(),
                    "d\n" => "D24\n".into(),
                    _ => {
                        assert!(
                            line.len() == 6 && matches!(line.as_bytes()[0], b'h' | b't'),
                            "unexpected scene settings peer {line:?}"
                        );
                        record.lock().unwrap().push(line.trim().to_owned());
                        let frame = u16::from_str_radix(&line[1..5], 16).unwrap();
                        let value = match frame {
                            0x0199 => Some(8),
                            0x01f7 => Some(0),
                            0x01f9 => Some(0xc3),
                            0x01fb => Some(128),
                            0x01fc => Some(dtr + 1),
                            0x01f8 => Some(0x20),
                            0x0190 => Some(4),
                            0x01a0 => {
                                selected = false;
                                Some(100)
                            }
                            0x01b0 => {
                                selected = true;
                                Some(current.load(Ordering::SeqCst))
                            }
                            0x01fa => {
                                let value = if selected {
                                    match dtr {
                                        240 => u16::from(stored_type.load(Ordering::SeqCst)),
                                        226 => stored_tc,
                                        _ => 65535,
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
                                low = value as u8;
                                Some(if selected && (233..=240).contains(&dtr) {
                                    value as u8
                                } else {
                                    (value >> 8) as u8
                                })
                            }
                            0x0198 => Some(low),
                            0xc108 => None,
                            0x0150 => {
                                assert!(line.starts_with('t'), "Remove was not twice-sent");
                                if fault != 1 {
                                    current.store(255, Ordering::SeqCst);
                                    stored_type.store(255, Ordering::SeqCst);
                                    stored_tc = 65535;
                                }
                                None
                            }
                            0x01ee => {
                                assert!(selected, "temporary copy used actual report");
                                temp_type = stored_type.load(Ordering::SeqCst);
                                temp_tc = stored_tc;
                                None
                            }
                            0x01e7 => {
                                temp_type = 0x20;
                                temp_tc = u16::from_be_bytes([dtr1, dtr]);
                                None
                            }
                            0x0140 => {
                                assert!(line.starts_with('t'), "Store was not twice-sent");
                                if fault == 3 {
                                    signal.notify_one();
                                    resume.notified().await;
                                }
                                if fault != 2 && fault != 4 {
                                    current.store(dtr, Ordering::SeqCst);
                                    if temp_type != 255 {
                                        stored_type.store(temp_type, Ordering::SeqCst);
                                        stored_tc = temp_tc;
                                    }
                                    temp_type = 255;
                                    temp_tc = 65535;
                                }
                                if fault == 4 {
                                    write.write_all(b"X\n").await.unwrap();
                                    line.clear();
                                    continue;
                                }
                                None
                            }
                            raw if raw >> 8 == 0xa3 => {
                                dtr = raw as u8;
                                None
                            }
                            raw if raw >> 8 == 0xc3 => {
                                dtr1 = raw as u8;
                                None
                            }
                            _ => {
                                panic!("scene editing activated or sent unexpected frame {line:?}")
                            }
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
            level,
            kind,
            frames,
            held,
            release,
            task,
        }
    }
    pub fn settings(&self) -> Vec<String> {
        self.frames
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.starts_with('t') || s.as_str() == "h01E7" || s.as_str() == "h01EE")
            .cloned()
            .collect()
    }
}
