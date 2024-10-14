#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        println!("{} {}", "[WARN]".color(Color::BrightYellow).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        println!("{} {}", "[INFO]".color(Color::BrightBlue).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("{} {}", "[ERROR]".color(Color::BrightRed).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        println!("{} {}", "[SUCCESS]".color(Color::BrightGreen).bold(), format!($($arg)*))
    };
}
