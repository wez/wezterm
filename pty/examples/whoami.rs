//! This is a conceptually simple example that spawns the `whoami` program
//! to print your username.  It is made more complex because there are multiple
//! pipes involved and it is easy to get blocked/deadlocked if care and attention
//! is not paid to those pipes!
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

fn main() {
    let pty_system = NativePtySystem::default();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let cmd = CommandBuilder::new("whoami");
    let mut child = pair.slave.spawn_command(cmd).unwrap();
    // Release any handles owned by the slave: we don't need it now
    // that we've spawned the child.
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().unwrap();
    println!("child status: {:?}", child.wait().unwrap());
    // We hold handles on the pty.  Now that the child is complete
    // there are no processes remaining that will write to it until
    // we spawn more.  We're not going to do that in this example,
    // so we should close it down.  If we didn't drop it explicitly
    // here, then the attempt to read its output would block forever
    // waiting for a future child that will never be spawned.
    drop(pair.master);

    // Consume the output from the child
    let mut s = String::new();
    reader.read_to_string(&mut s).unwrap();

    // We print with escapes escaped because the windows conpty
    // implementation synthesizes title change escape sequences
    // in the output stream and it can be confusing to see those
    // printed out raw in another terminal.
    print!("output: ");
    for c in s.escape_debug() {
        print!("{}", c);
    }
}
