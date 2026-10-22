use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub(super) struct Peer {
    pub path: PathBuf,
    pub frames: Arc<std::sync::Mutex<Vec<u16>>>,
    pub model: Arc<std::sync::Mutex<crate::emulator::model::Model>>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub fn new(capture: Option<(u8, u8)>, colour: bool) -> Self {
        let mut profile = serde_json::to_value(scenario(73)).unwrap();
        if colour {
            profile["buses"][0]["outputs"][0]["device_type"] = json!(8);
            profile["buses"][0]["outputs"][0]["colour"] = json!({"active":"temperature","auto_activation":true,"temperature":{"value":250,"coolest":200,"warmest":400,"physical_coolest":153,"physical_warmest":500}});
        }
        let mut model =
            crate::emulator::model::Model::new(serde_json::from_value(profile).unwrap()).unwrap();
        if colour {
            for frame in [0xa35e, 0xc301, 0xc108, 0x01e7] {
                assert_eq!(
                    model.transmit(0, frame, 0),
                    crate::emulator::model::Reply::None
                );
            }
        }
        let model = Arc::new(std::sync::Mutex::new(model));
        let frames = Arc::new(std::sync::Mutex::new(Vec::new()));
        let pty = super::super::super::hat::tests::Pty::new();
        let path = pty.path.clone();
        let (record, physical) = (frames.clone(), model.clone());
        let task = tokio::spawn(async move {
            // Retain the slave descriptor as well as the master inside this task.
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut line = Vec::new();
            loop {
                if read.read_until(b'\n', &mut line).await.unwrap() == 0 {
                    return;
                }
                let reply = match line.as_slice() {
                    b"v\n" => b"V013204\n".to_vec(),
                    b"d\n" => b"D24\n".to_vec(),
                    command if command.first() == Some(&b'h') && command.len() == 6 => {
                        let frame =
                            u16::from_str_radix(std::str::from_utf8(&command[1..5]).unwrap(), 16)
                                .unwrap();
                        record.lock().unwrap().push(frame);
                        let reply = match (frame, capture) {
                            (0x01a0, Some((level, _))) => {
                                crate::emulator::model::Reply::Value(level)
                            }
                            (0x0190, Some((_, status))) => {
                                crate::emulator::model::Reply::Value(status)
                            }
                            _ => physical.lock().unwrap().transmit(0, frame, 0),
                        };
                        match reply {
                            crate::emulator::model::Reply::Value(v) => {
                                format!("J{v:02X}\n").into_bytes()
                            }
                            crate::emulator::model::Reply::None => b"N\n".to_vec(),
                            crate::emulator::model::Reply::Collision => b"X\n".to_vec(),
                        }
                    }
                    other => panic!("unexpected identification peer command {other:?}"),
                };
                write.write_all(&reply).await.unwrap();
                line.clear();
            }
        });
        Self {
            path,
            frames,
            model,
            task,
        }
    }
    pub async fn close(mut self) {
        self.task.abort();
        assert!((&mut self.task).await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
