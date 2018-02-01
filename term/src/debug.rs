macro_rules! debug {
    ($($arg:tt)*) => (if cfg!(feature="debug-escape-sequences") {
        println!($($arg)*)
    })
}
