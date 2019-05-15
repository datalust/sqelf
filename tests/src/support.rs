use std::{net::UdpSocket, thread, time::Duration};

use sqelf::{process, receive, server};

pub use serde_json::Value;

pub struct ToReceive {
    pub count: usize,
    pub when_sending: Vec<Vec<u8>>,
}

pub fn expect(to_receive: ToReceive, check: impl Fn(&[Value])) {
    let ToReceive {
        count,
        when_sending,
    } = to_receive;

    assert!(
        when_sending.len() >= count,
        "cannot receive the expected number of messages based on the datagrams to send"
    );

    let (tx, rx) = crossbeam_channel::unbounded();

    // Build a server
    let (server, handle) = server::build(
        server::Config {
            bind: "0.0.0.0:12202".into(),
            bind_handle: true,
            wait_on_stdin: false,
            ..Default::default()
        },
        {
            let mut receive = receive::build(receive::Config {
                ..Default::default()
            });

            move |src| receive.decode(src)
        },
        {
            let process = process::build(process::Config {
                ..Default::default()
            });

            move |msg| {
                process.with_clef(msg, |clef| {
                    let json = serde_json::to_value(clef)?;
                    tx.send(json)?;

                    Ok(())
                })
            }
        },
    )
    .expect("failed to build server");

    let handle = handle.expect("no server handle");
    let server = thread::spawn(move || server.run().expect("failed to run server"));

    // Send our datagrams
    let sock = UdpSocket::bind("127.0.0.1:0").expect("failed to bind client socket");
    for dgram in when_sending {
        sock.send_to(&dgram, "127.0.0.1:12202")
            .expect("failed to send datagram");
    }

    // Wait for the messages to be processed
    let mut received = Vec::with_capacity(count);
    while received.len() < count {
        let msg = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("failed to receive a message");
        received.push(msg);
    }

    // Close the server
    handle.close();
    server.join().expect("failed to run server");

    // Check the messages received
    check(&received);
}

macro_rules! dgrams {
    ($(..$dgrams:expr),+) => {{
        let mut v = Vec::new();

        $(
            v.extend($dgrams);
        )+

        v
    }};
    ({$($json:tt)*}) => {{
        let v = serde_json::to_vec(&json!({$($json)*})).unwrap();
        vec![v]
    }}
}

macro_rules! cases {
    ($($case:ident),+) => {
        $(
            mod $case;
        )+

        pub(crate) fn test_all() {
            $(
                println!("running {}...", stringify!($case));
                self::$case::test();
            )+
        }
    }
}
