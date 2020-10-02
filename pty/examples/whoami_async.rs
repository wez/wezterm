use anyhow::anyhow;
use futures::prelude::*;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

// This example shows how to use the `smol` crate to use portable_pty
// in an asynchronous application.

fn main() -> anyhow::Result<()> {
    smol::block_on(async {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let cmd = CommandBuilder::new("whoami");

        // Move the slave to another thread to block and spawn a
        // command.
        // Note that this implicitly drops slave and closes out
        // file handles which is important to avoid deadlock
        // when waiting for the child process!
        let slave = pair.slave;
        let mut child = smol::unblock(move || slave.spawn_command(cmd)).await?;

        let reader = pair.master.try_clone_reader()?;

        // We hold handles on the pty.  Now that the child is complete
        // there are no processes remaining that will write to it until
        // we spawn more.  We're not going to do that in this example,
        // so we should close it down.  If we didn't drop it explicitly
        // here, then the attempt to read its output would block forever
        // waiting for a future child that will never be spawned.
        drop(pair.master);

        let mut lines = smol::io::BufReader::new(smol::Unblock::new(reader)).lines();
        while let Some(line) = lines.next().await {
            let line = line.map_err(|e| anyhow!("problem reading line: {}", e))?;
            // We print with escapes escaped because the windows conpty
            // implementation synthesizes title change escape sequences
            // in the output stream and it can be confusing to see those
            // printed out raw in another terminal.
            print!("output: len={} ", line.len());
            for c in line.escape_debug() {
                print!("{}", c);
            }
            println!();
        }

        // Note that we're waiting until after we've read the output
        // to call `wait` on the process.
        // On macOS Catalina, waiting on the process seems to prevent
        // its output from making it into the pty.
        println!(
            "child status: {:?}",
            smol::unblock(move || child
                .wait()
                .map_err(|e| anyhow!("waiting for child: {}", e)))
            .await?
        );
        Ok(())
    })
}
