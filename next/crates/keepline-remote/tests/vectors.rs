//! Fixed inputs and the exact bytes they give, kept in `vectors/v1.json` for clients in
//! other languages to check themselves against. Run with `KEEPLINE_WRITE_VECTORS=1` to
//! write the file after a deliberate change; otherwise the bytes must not move.
use keepline_remote::{
    link::Link,
    secure::{self, Code, Message},
    wire::{self, bin},
};
use serde_json::{json, Value};

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn seq(start: u8) -> [u8; 32] {
    std::array::from_fn(|i| start.wrapping_add(i as u8))
}

fn vectors() -> Value {
    let code = Code::from_bytes(seq(0x00));
    let key = Code::from_bytes(seq(0x40));
    let (e_phone, e_machine) = (seq(0x80), seq(0xc0));
    let token = code.token();

    let (hs, first) = secure::initiate_with(&key, Some(&e_phone)).unwrap();
    let (mut machine, answer) = secure::respond_with(&key, &first, Some(&e_machine)).unwrap();
    let mut phone = hs.finish(&answer).unwrap();
    let hello = b"{\"id\":1,\"op\":\"hello\"}\n";
    let reply = b"{\"id\":1,\"ok\":{}}\n";
    let m1 = phone.data(hello).unwrap();
    let m2 = machine.data(reply).unwrap();
    let m3 = phone.ack(reply.len() as u32).unwrap();
    // Each side opens what the other sealed, in order.
    assert_eq!(machine.open(&m1).unwrap(), Message::Data(hello.to_vec()));
    assert_eq!(phone.open(&m2).unwrap(), Message::Data(reply.to_vec()));
    assert_eq!(machine.open(&m3).unwrap(), Message::Ack(reply.len() as u32));

    let link = Link {
        relay: "https://relay.example.com".into(),
        code: code.clone(),
        device: "a1b2c3d4e5f6".into(),
        key: key.clone(),
        machine: "2fd56880-55df-4677-86df-7c3496a1b2c3".into(),
        name: "Lin 的 Mac".into(),
    };
    json!({
        "about": "keepline remote v1 / pairing link v2. Bytes are lowercase hex; codes are base64url without padding.",
        "machine_code": { "bytes": hex(&code.bytes()), "text": code.text() },
        "token": token,
        "token_hash": wire::token_hash(&token),
        "device_key": { "bytes": hex(&key.bytes()), "text": key.text(), "psk": hex(&key.psk()) },
        "noise": {
            "pattern": secure::PATTERN,
            "prologue": "keepline remote 1",
            "phone_ephemeral_private": hex(&e_phone),
            "machine_ephemeral_private": hex(&e_machine),
            "handshake_1_phone": hex(&first),
            "handshake_2_machine": hex(&answer),
        },
        "messages": [
            { "from": "phone", "kind": "data", "plain": String::from_utf8_lossy(hello), "sealed": hex(&m1) },
            { "from": "machine", "kind": "data", "plain": String::from_utf8_lossy(reply), "sealed": hex(&m2) },
            { "from": "phone", "kind": "ack", "n": reply.len(), "sealed": hex(&m3) },
        ],
        "relay_frames": [
            { "kind": bin::WBIN, "id": "w1", "payload": hex(&m1), "frame": hex(&wire::encode(bin::WBIN, "w1", &m1)) },
        ],
        "link": { "text": link.text(), "url": link.url().unwrap() },
    })
}

#[test]
fn the_bytes_match_the_vectors_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors/v1.json");
    let now = vectors();
    if std::env::var_os("KEEPLINE_WRITE_VECTORS").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(&now).unwrap() + "\n").unwrap();
    }
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        now, kept,
        "the protocol's bytes changed; a client built on v1 breaks"
    );
}

#[test]
fn the_vectors_link_parses_back() {
    let v = vectors();
    let link = Link::parse(v["link"]["text"].as_str().unwrap()).unwrap();
    assert_eq!(link.code.text(), v["machine_code"]["text"]);
    assert_eq!(link.name, "Lin 的 Mac");
}
