//! `keepline-relay [--listen <addr>] [--key <key>]`: runs the relay. Put it behind an
//! HTTPS proxy (Caddy, nginx) on a public host; machines and phones both dial in.
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut listen = std::env::var("RELAY_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let mut config = keepline_relay::server::Config {
        key: std::env::var("RELAY_REGISTRATION_KEY")
            .ok()
            .filter(|k| !k.is_empty()),
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| io::Error::other(format!("{arg} 需要一个值")))
        };
        match arg.as_str() {
            "--listen" => listen = value()?,
            "--key" => config.key = Some(value()?),
            _ => {
                return Err(io::Error::other(
                    "usage: keepline-relay [--listen <地址:端口>] [--key <注册密钥>]",
                ))
            }
        }
    }
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    let gate = if config.key.is_some() { "key" } else { "open" };
    // Tests and scripts read the bound address from this line.
    println!(
        "keepline-relay listening on {} ({gate})",
        listener.local_addr()?
    );
    keepline_relay::server::serve(listener, config).await
}
