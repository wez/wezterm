use portable_pty::awaitable::native_pty_system;
use portable_pty::{CommandBuilder, PtySize};
use tokio::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .await?;

    let cmd = CommandBuilder::new("whoami");
    let child = pair.slave.spawn_command(cmd).await?;
    // Release any handles owned by the slave: we don't need it now
    // that we've spawned the child, and if we don't drop it, we'll
    // block a one of the awaits we perform below.
    drop(pair.slave);

    let reader = pair.master.try_clone_reader()?;
    println!("child status: {:?}", child.await?);

    // We hold handles on the pty.  Now that the child is complete
    // there are no processes remaining that will write to it until
    // we spawn more.  We're not going to do that in this example,
    // so we should close it down.  If we didn't drop it explicitly
    // here, then the attempt to read its output would block forever
    // waiting for a future child that will never be spawned.
    drop(pair.master);

    let mut lines = tokio::io::BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        // We print with escapes escaped because the windows conpty
        // implementation synthesizes title change escape sequences
        // in the output stream and it can be confusing to see those
        // printed out raw in another terminal.
        print!("output: ");
        for c in line.escape_debug() {
            print!("{}", c);
        }
        println!();
    }
    Ok(())
}
