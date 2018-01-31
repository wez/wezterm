macro_rules! debug {
    ($($arg:tt)*) => (if cfg!(debug_assertions) { println!($($arg)*) })
}
