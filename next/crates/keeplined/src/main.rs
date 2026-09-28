fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("keeplined: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => {
            let runtime = runtime_dir(args)?;
            keeplined::serve(std::path::Path::new(&runtime))
        }
        Some("--version") => {
            println!(
                "keeplined protocol {}.{} experimental",
                keeplined::PROTOCOL_MAJOR,
                keeplined::PROTOCOL_MINOR
            );
            Ok(())
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "experimental managed PTY daemon (protocol major {}). \
                 the keepline command does not start it. \
                 usage: keeplined serve --runtime <dir>",
                keeplined::PROTOCOL_MAJOR
            ),
        )),
    }
}

fn runtime_dir(mut args: impl Iterator<Item = String>) -> std::io::Result<String> {
    let mut runtime = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runtime" => {
                runtime = Some(args.next().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "missing --runtime directory",
                    )
                })?);
            }
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown argument {other}"),
                ));
            }
        }
    }
    runtime.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "missing --runtime directory",
        )
    })
}
