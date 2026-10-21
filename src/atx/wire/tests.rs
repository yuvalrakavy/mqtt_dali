use super::*;

#[test]
fn manufacturer_wire_vectors_preserve_bus_width_status_and_firmware_text() {
    for (line, bus, kind) in [
        ("HFEFE", 0, Kind::Dali(DaliFrame::Bits16(0xfefe))),
        ("1J06", 1, Kind::Dali(DaliFrame::Bits8(6))),
        ("2L123456", 2, Kind::Dali(DaliFrame::Bits24(0x123456))),
        ("3M01020304", 3, Kind::Dali(DaliFrame::Bits25(0x01020304))),
        ("0N", 0, Kind::NoResponse),
        ("3N", 3, Kind::NoResponse),
        ("2X4", 2, Kind::ReceiveCollision(Some(4))),
        ("1Z02", 1, Kind::TransmitCollision(Some(2))),
        ("Z", 0, Kind::TransmitCollision(None)),
        ("", 0, Kind::UnattributedCollision),
        (
            "3D24",
            3,
            Kind::BusStatus {
                status: 2,
                channels: 4,
            },
        ),
        (
            "D01",
            0,
            Kind::BusStatus {
                status: 0,
                channels: 1,
            },
        ),
        ("P0000001", 0, Kind::Power { raw: 1 }),
        (
            "V013204",
            0,
            Kind::Version(Version {
                hardware: "01".into(),
                firmware: "32".into(),
                variant: "04".into(),
            }),
        ),
        (
            "B021804",
            0,
            Kind::Boot(Version {
                hardware: "02".into(),
                firmware: "18".into(),
                variant: "04".into(),
            }),
        ),
    ] {
        assert_eq!(
            decode(line.as_bytes()).unwrap(),
            Packet { bus, kind },
            "{line}"
        );
    }
}

#[test]
fn malformed_lines_are_errors_with_no_slicing_or_unicode_panics() {
    for line in [
        "0",
        "3",
        "H",
        "H123",
        "H12345",
        "L1234",
        "MFFFFFFFF",
        "N00",
        "X123",
        "4N",
        "v013204",
        "D2",
        "D214",
        "V0132",
        "JZZ",
        "\r",
        "é",
        " H1234",
        "J01\n",
    ] {
        let result = std::panic::catch_unwind(|| decode(line.as_bytes()));
        assert!(result.unwrap().is_err(), "accepted invalid line {line:?}");
    }
}

#[test]
fn commands_encode_all_buses_exact_width_and_repeat_without_touching_hardware() {
    for (command, text) in [
        (
            Command::Send {
                bus: 0,
                frame: DaliFrame::Bits16(0xfefe),
                repeat: false,
            },
            "hFEFE\n",
        ),
        (
            Command::Send {
                bus: 3,
                frame: DaliFrame::Bits16(0xa500),
                repeat: true,
            },
            "3tA500\n",
        ),
        (
            Command::Send {
                bus: 2,
                frame: DaliFrame::Bits8(1),
                repeat: false,
            },
            "2j01\n",
        ),
        (
            Command::Send {
                bus: 1,
                frame: DaliFrame::Bits24(0x1234),
                repeat: false,
            },
            "1l001234\n",
        ),
        (
            Command::Send {
                bus: 0,
                frame: DaliFrame::Bits25(0x1000000),
                repeat: false,
            },
            "m01000000\n",
        ),
        (Command::BusStatus { bus: 2 }, "2d\n"),
        (Command::Version, "v\n"),
    ] {
        assert_eq!(command.encode().unwrap(), text.as_bytes());
    }
    for command in [
        Command::BusStatus { bus: 4 },
        Command::Send {
            bus: 4,
            frame: DaliFrame::Bits16(0),
            repeat: false,
        },
        Command::Send {
            bus: 0,
            frame: DaliFrame::Bits24(1 << 24),
            repeat: false,
        },
        Command::Send {
            bus: 0,
            frame: DaliFrame::Bits25(1 << 25),
            repeat: false,
        },
        Command::Send {
            bus: 0,
            frame: DaliFrame::Bits8(0),
            repeat: true,
        },
    ] {
        assert!(
            command.encode().is_err(),
            "accepted invalid command {command:?}"
        );
    }
}

#[test]
fn partial_and_multiple_lines_accept_crlf_without_discarding_idle_events() {
    let mut framer = Framer::default();
    let now = Instant::now();
    assert!(framer.feed(b"1L010", now).is_empty());
    let packets = framer.feed(b"203\r\n2J06\nN\n", now);
    assert_eq!(
        packets,
        vec![decode(b"1L010203"), decode(b"2J06"), decode(b"N")]
    );
    assert_eq!(framer.deadline(), None);
    assert_eq!(
        framer.feed(b"\n", now),
        vec![Ok(Packet {
            bus: 0,
            kind: Kind::UnattributedCollision
        })]
    );
}

#[test]
fn oversized_lines_are_bounded_and_recover_only_at_the_next_newline() {
    let mut framer = Framer::default();
    let now = Instant::now();
    let oversized = vec![b'L'; MAX_LINE_BYTES + 1000];
    assert_eq!(
        framer.feed(&oversized, now),
        vec![Err(DecodeError::LineTooLong)]
    );
    assert!(framer.feed(b"J00", now).is_empty());
    assert_eq!(framer.feed(b"\nL010203\n", now), vec![decode(b"L010203")]);
}

#[test]
fn trickling_lines_expire_by_total_time_and_later_events_still_decode() {
    let mut framer = Framer::default();
    let now = Instant::now();
    assert!(framer.feed(b"L01", now).is_empty());
    assert_eq!(framer.deadline(), Some(now + LINE_DEADLINE));
    assert!(framer
        .feed(b"02", now + Duration::from_millis(900))
        .is_empty());
    assert_eq!(
        framer.expire(now + LINE_DEADLINE),
        Some(DecodeError::LineDeadline)
    );
    assert_eq!(framer.expire(now + LINE_DEADLINE), None);
    assert_eq!(
        framer.feed(b"03\n1N\n", now + LINE_DEADLINE),
        vec![decode(b"1N")]
    );
    assert_eq!(framer.deadline(), None);
}
