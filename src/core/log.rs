#[macro_export]
macro_rules! warnln {
    ($($arg:tt)*) => {
        println!("{} {}", "[WARN]".color(Color::BrightYellow).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! infoln {
    ($($arg:tt)*) => {
        println!("{} {}", "[INFO]".color(Color::BrightBlue).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! errorln {
    ($($arg:tt)*) => {
        eprintln!("{} {}", "[ERROR]".color(Color::BrightRed).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! successln {
    ($($arg:tt)*) => {
        println!("{} {}", "[SUCCESS]".color(Color::BrightGreen).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        print!("{} {}", "[WARN]".color(Color::BrightYellow).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        print!("{} {}", "[INFO]".color(Color::BrightBlue).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        eprint!("{} {}", "[ERROR]".color(Color::BrightRed).bold(), format!($($arg)*))
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        print!("{} {}", "[SUCCESS]".color(Color::BrightGreen).bold(), format!($($arg)*))
    };
}
