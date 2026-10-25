use pyxamp_daemon::{
    protocol::*,
    socket::{read_frame, runtime_socket},
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::UnixStream,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        !args.is_empty(),
        "Usage: pyxampctl state|watch|command '<command payload JSON>'|request '<request payload JSON>'"
    );
    let stream = UnixStream::connect(runtime_socket()?).await?;
    let (read, mut write) = stream.into_split();
    let mut read = BufReader::new(read);
    let mut hello = serde_json::to_vec(&ClientMessage::Hello { version: VERSION })?;
    hello.push(b'\n');
    write.write_all(&hello).await?;
    let mut issued = false;
    loop {
        let bytes = read_frame(&mut read)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Daemon disconnected"))?;
        let message: ServerMessage = serde_json::from_slice(&bytes)?;
        match (&message, args[0].as_str()) {
            (ServerMessage::State(_), "state") => {
                println!("{}", String::from_utf8(bytes)?);
                return Ok(());
            }
            (ServerMessage::Ready { .. }, "command") if !issued => {
                let command: Command = serde_json::from_str(args.get(1).ok_or_else(|| {
                    anyhow::anyhow!("command requires an explicit captured target payload")
                })?)?;
                command
                    .validate()
                    .map_err(|e| anyhow::anyhow!("{}: {}", e.code, e.message))?;
                let mut bytes = serde_json::to_vec(&ClientMessage::Command(command))?;
                bytes.push(b'\n');
                write.write_all(&bytes).await?;
                issued = true;
            }
            (ServerMessage::Ready { .. }, "request") if !issued => {
                let request: Request = serde_json::from_str(args.get(1).ok_or_else(|| {
                    anyhow::anyhow!(
                        "request requires a payload such as {{\"id\":\"r1\",\"action\":{{...}}}}"
                    )
                })?)?;
                request
                    .validate()
                    .map_err(|e| anyhow::anyhow!("{}: {}", e.code, e.message))?;
                let mut bytes = serde_json::to_vec(&ClientMessage::Request(request))?;
                bytes.push(b'\n');
                write.write_all(&bytes).await?;
                issued = true;
            }
            (ServerMessage::Page(_), "request") => println!("{}", String::from_utf8(bytes)?),
            (ServerMessage::Result { outcome, .. }, "command" | "request") => {
                println!("{}", String::from_utf8(bytes)?);
                anyhow::ensure!(matches!(outcome, Outcome::Accepted(_)), "Command rejected");
                return Ok(());
            }
            (ServerMessage::Error(error), _) => anyhow::bail!("{}: {}", error.code, error.message),
            (_, "watch") => println!("{}", String::from_utf8(bytes)?),
            (_, "state" | "command" | "request") => {}
            _ => anyhow::bail!(
                "Usage: pyxampctl state|watch|command '<command payload JSON>'|request '<request payload JSON>'"
            ),
        }
    }
}
