//! Literal peer scripts validate the executing caller independently of the model.
use super::*;
use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(super) struct SettingsPeer {
    pub path: PathBuf,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for SettingsPeer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl SettingsPeer {
    pub fn new(script: Vec<(String, String)>) -> Self {
        let pty = crate::service::hat::tests::Pty::new();
        let path = pty.path.clone();
        let task = tokio::spawn(async move {
            // Capture the whole PTY, including the slave keeper, before borrowing
            // its peer. Disjoint capture of only peer would drop the slave early.
            let mut pty = pty;
            let (read, mut write) = tokio::io::split(&mut pty.peer);
            let mut read = BufReader::new(read);
            let mut script = VecDeque::from(script);
            loop {
                let mut line = String::new();
                if read.read_line(&mut line).await.unwrap() == 0 {
                    return;
                }
                let reply = match line.as_str() {
                    "v\n" => "V013204\n".to_string(),
                    "d\n" => "D24\n".to_string(),
                    "h01A0\n" => "J64\n".to_string(),
                    "h0190\n" => "J00\n".to_string(),
                    _ => {
                        let (expected, reply) = script
                            .pop_front()
                            .expect("unexpected literal settings command");
                        assert_eq!(line, expected, "literal settings command mismatch");
                        reply
                    }
                };
                write.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        Self { path, task }
    }
}
pub(super) fn script(instance: u8, kind: u8, enabled: &str) -> Vec<(String, String)> {
    let mut script = Vec::new();
    for (opcode, value) in [
        (0x80, format!("J{kind:02X}\n")),
        (0x86, enabled.into()),
        (0x84, "J04\n".into()),
        (0x8b, "J02\n".into()),
        (0x88, "JFF\n".into()),
        (0x89, "JFF\n".into()),
        (0x8a, "JFF\n".into()),
        (0x90, "JFF\n".into()),
    ] {
        script.push((format!("l01{instance:02X}{opcode:02X}\n"), value));
    }
    if kind == 31 {
        script.extend([
            (format!("l01{instance:02X}91\n"), "JFF\n".into()),
            (format!("l01{instance:02X}92\n"), "JFF\n".into()),
        ]);
    }
    script.push((format!("l01{instance:02X}80\n"), format!("J{kind:02X}\n")));
    script
}
