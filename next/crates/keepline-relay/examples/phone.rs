//! A phone in a terminal, and the smallest complete client of a Keepline machine on a
//! relay: `cargo run --example phone -- '<pairing link>'`. Type remote-protocol requests
//! as JSON lines on stdin (`{"id":1,"op":"hello"}`); replies and stream messages come
//! out on stdout, one per line.
//!
//! Every client does the same four things:
//! 1. Open a WebSocket to `<relay>/c/<token>/keepline`, the token derived from the code.
//! 2. Send the Noise handshake's first message; finish with the machine's answer.
//! 3. Seal outgoing JSON lines as DATA; open incoming messages.
//! 4. Acknowledge DATA it has taken in, or the machine stops after a window.
use futures_util::{SinkExt, StreamExt};
use keepline_relay::secure::{self, Code, Message as Secure};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let link = std::env::args()
        .nth(1)
        .ok_or("usage: phone '<keepline://pair?…>'")?;
    let field = |name: &str| {
        link.split(['?', '&'])
            .find_map(|kv| kv.strip_prefix(&format!("{name}=")))
            .map(unescape)
    };
    let relay = field("relay").ok_or("the link has no relay")?;
    let code = field("code")
        .and_then(|c| Code::parse(&c))
        .ok_or("the link has no code")?;
    let base = relay
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    let url = format!("{}/c/{}/keepline", base.trim_end_matches('/'), code.token());
    let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut sink, mut stream) = ws.split();

    let (hs, first) = secure::initiate(&code)?;
    sink.send(Message::binary(first)).await?;
    let mut channel = loop {
        match stream.next().await.ok_or("relay closed")?? {
            Message::Binary(answer) => break hs.finish(&answer)?,
            Message::Close(f) => return Err(format!("closed: {f:?}").into()),
            _ => continue,
        }
    };
    eprintln!("connected to {relay}");

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                let Some(line) = line? else { break };
                let line = format!("{line}\n");
                for chunk in line.as_bytes().chunks(secure::CHUNK) {
                    sink.send(Message::binary(channel.data(chunk)?)).await?;
                }
            }
            m = stream.next() => match m.ok_or("relay closed")?? {
                Message::Binary(b) => {
                    if let Secure::Data(d) = channel.open(&b)? {
                        stdout.write_all(&d).await?;
                        stdout.flush().await?;
                        sink.send(Message::binary(channel.ack(d.len() as u32)?)).await?;
                    }
                }
                Message::Close(f) => return Err(format!("closed: {f:?}").into()),
                _ => (),
            },
        }
    }
    Ok(())
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1..i + 3)) {
            (b'%', Some(h))
                if std::str::from_utf8(h).is_ok_and(|h| u8::from_str_radix(h, 16).is_ok()) =>
            {
                out.push(u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap());
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
