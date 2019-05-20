use portable_pty::{CommandBuilder, PtySize, PtySystemSelection};

fn main() {
    let pty_system = PtySystemSelection::default().get().unwrap();

    let (master, slave) = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let cmd = CommandBuilder::new("whoami");
    let mut child = slave.spawn_command(cmd).unwrap();

    let mut s = String::new();
    master
        .try_clone_reader()
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    println!("output: {}", s);
    child.wait().unwrap();
}
