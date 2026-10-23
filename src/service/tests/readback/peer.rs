use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub(super) struct Peer {
    pub path: PathBuf,
    pub held: Arc<tokio::sync::Notify>,
    pub release: Arc<tokio::sync::Notify>,
    pub commands: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    pub fn start(kind: u8, resolution: u8, bytes: Vec<u8>, delay: Duration, hold: bool) -> Self {
        let pty = super::super::super::hat::tests::Pty::new();
        let path = pty.path.clone();
        let held = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (signal, resume, recorded) = (held.clone(), release.clone(), commands.clone());
        let task = tokio::spawn(async move {
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            let mut cursor = 0;
            let mut held_once = false;
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                assert!(line.len() < 32);
                recorded.lock().unwrap().push(line.clone());
                let response = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    b"h01A0\n" => b"J64\n".to_vec(),
                    b"h0190\n" => b"J04\n".to_vec(),
                    b"h0199\n" => b"J00\n".to_vec(),
                    command if command.first() == Some(&b'l') && command.len() == 8 => {
                        let opcode =
                            u8::from_str_radix(std::str::from_utf8(&command[5..7]).unwrap(), 16)
                                .unwrap();
                        if opcode == 0x8c && hold && !held_once {
                            held_once = true;
                            write.write_all(b"L008000\n").await.unwrap();
                            signal.notify_one();
                            resume.notified().await;
                        }
                        if !delay.is_zero() {
                            tokio::time::sleep(delay).await;
                        }
                        let value = match opcode {
                            0x80 => Some(kind),
                            0x81 => Some(resolution),
                            // Target refresh also inspects this input's indicator.
                            // This literal device explicitly has no features.
                            0x8e => Some(0xfe),
                            0x8c => {
                                cursor = 1;
                                bytes.first().copied()
                            }
                            0x8d => {
                                let value = bytes.get(cursor).copied();
                                cursor += 1;
                                value
                            }
                            _ => panic!("unexpected input opcode {opcode:X}"),
                        };
                        value
                            .map_or_else(|| b"N\n".to_vec(), |v| format!("J{v:02X}\n").into_bytes())
                    }
                    other => panic!("unexpected independent readback peer command {other:?}"),
                };
                write.write_all(&response).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            held,
            release,
            commands,
            task,
        }
    }
    pub async fn close(&mut self) {
        self.task.abort();
        assert!((&mut self.task).await.unwrap_err().is_cancelled());
    }
}
