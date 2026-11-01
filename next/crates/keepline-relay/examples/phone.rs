//! A phone in a terminal, and the smallest complete client of a Keepline machine on a
//! relay: `cargo run --example phone -- '<pairing link>'`. Type remote-protocol requests
//! as JSON lines on stdin (`{"id":1,"op":"hello"}`); replies and stream messages come
//! out on stdout, one per line.
//!
//! Every client does the same four things:
//! 1. Open a WebSocket to `<relay>/c/<token>/keepline?d=<device>`: the token derived from
//!    the machine's code, the device id from the link.
//! 2. Send the Noise handshake's first message, keyed by the device's key; finish with
//!    the machine's answer.
//! 3. Seal outgoing JSON lines as DATA; open incoming messages.
//! 4. Acknowledge DATA it has taken in, or the machine stops after a window.
use futures_util::{SinkExt, StreamExt};
use keepline_remote::{
    link::Link,
    secure::{self, Message as Secure},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::env::args()
        .nth(1)
        .ok_or("usage: phone '<keepline://pair?…>'")?;
    let link = Link::parse(&text)?;
    let (relay, key) = (&link.relay, &link.key);
    let url = link.url()?;
    let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut sink, mut stream) = ws.split();

    let (hs, first) = secure::initiate(key)?;
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
