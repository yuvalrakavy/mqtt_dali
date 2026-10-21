use super::*;
use crate::atx::monitor::{Event, Monitor};
use crate::atx::wire::{Command, DaliFrame, Kind, Packet};
use std::ffi::CStr;
use std::os::fd::FromRawFd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn pty() -> (OwnedFd, OwnedFd, PathBuf) {
    let mut master = -1;
    let mut slave = -1;
    let mut name = [0 as libc::c_char; 1024];
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                name.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let name = unsafe { CStr::from_ptr(name.as_ptr()) }.to_str().unwrap();
    (
        unsafe { OwnedFd::from_raw_fd(master) },
        unsafe { OwnedFd::from_raw_fd(slave) },
        PathBuf::from(name),
    )
}

#[tokio::test]
async fn production_serial_adapter_uses_raw_19200_and_preserves_idle_pty_events() {
    let (master, slave, path) = pty();
    let mut hat = Serial::from_owned_fd(master).unwrap();
    let serial = Serial::open(path).await.unwrap();
    let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), settings.as_mut_ptr()) },
        0
    );
    let settings = unsafe { settings.assume_init() };
    assert_eq!(unsafe { libc::cfgetispeed(&settings) }, libc::B19200);
    assert_eq!(
        settings.c_lflag & (libc::ICANON | libc::ECHO),
        0,
        "UART must be raw without canonical/echo processing"
    );
    assert_eq!(
        settings.c_iflag & (libc::IXON | libc::IXOFF | libc::ICRNL),
        0
    );
    assert_eq!(settings.c_cc[libc::VMIN], 1);
    let (monitor, mut events) = Monitor::start(serial);
    hat.write_all(b"3L123456\r\n").await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        event,
        Event::Packet {
            sequence: 1,
            packet: Ok(Packet {
                bus: 3,
                kind: Kind::Dali(DaliFrame::Bits24(0x123456))
            })
        }
    );
    monitor
        .write(&Command::Version, Duration::from_secs(1))
        .await
        .unwrap();
    let mut command = [0; 2];
    tokio::time::timeout(Duration::from_secs(1), hat.read_exact(&mut command))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        &command, b"v\n",
        "PTY canonical/echo/CR conversion altered the wire"
    );
    monitor.close().await.unwrap();
}

#[tokio::test]
async fn non_tty_files_and_missing_serial_paths_fail_without_creating_files() {
    let path = std::env::temp_dir().join(format!("atx-not-tty-{}", std::process::id()));
    std::fs::write(&path, b"unchanged").unwrap();
    let result = Serial::open(&path).await;
    assert!(result.is_err(), "accepted a non-tty file");
    assert_eq!(std::fs::read(&path).unwrap(), b"unchanged");
    std::fs::remove_file(&path).unwrap();
    assert!(Serial::open(&path).await.is_err());
    assert!(!path.exists());
}

#[tokio::test]
async fn transaction_dispatcher_separates_pty_input_events_from_query_completion() {
    use crate::atx::transactions::{BusReply, RepeatPolicy, Response, Transactions};
    let (master, _slave, path) = pty();
    let mut hat = Serial::from_owned_fd(master).unwrap();
    let (link, mut events) =
        Transactions::start(Serial::open(path).await.unwrap(), RepeatPolicy::Unverified);
    let clone = link.clone();
    let init = tokio::spawn(async move { clone.initialize(Duration::from_secs(2)).await });
    let mut bytes = [0; 2];
    tokio::time::timeout(Duration::from_secs(2), hat.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"v\n");
    hat.write_all(b"V013204\r\n").await.unwrap();
    assert_eq!(init.await.unwrap().unwrap().firmware, "32");
    let clone = link.clone();
    let query = tokio::spawn(async move {
        clone
            .execute(
                Command::Send {
                    bus: 2,
                    frame: DaliFrame::Bits16(0x0790),
                    repeat: false,
                },
                Duration::from_secs(2),
            )
            .await
    });
    let mut bytes = [0; 7];
    tokio::time::timeout(Duration::from_secs(2), hat.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"2h0790\n");
    hat.write_all(b"2L020401\n1J11\n2J42\n").await.unwrap();
    assert_eq!(
        query.await.unwrap().unwrap(),
        Response::Dali(vec![BusReply::Value(0x42)])
    );
    let mut input_seen = false;
    while let Ok(event) = events.try_recv() {
        input_seen |= matches!(
            event,
            Event::Packet {
                packet: Ok(Packet {
                    bus: 2,
                    kind: Kind::Dali(DaliFrame::Bits24(0x020401))
                }),
                ..
            }
        );
    }
    assert!(
        input_seen,
        "PTY input event was consumed as a query acknowledgement"
    );
    link.close().await.unwrap();
}
