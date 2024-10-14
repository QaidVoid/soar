use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum Color {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl Color {
    pub fn to_ansi_code(self, is_bg: bool) -> &'static str {
        match self {
            Color::Reset => "\x1B[0m",
            Color::Black => {
                if is_bg {
                    "\x1B[40m"
                } else {
                    "\x1B[30m"
                }
            }
            Color::Red => {
                if is_bg {
                    "\x1B[41m"
                } else {
                    "\x1B[31m"
                }
            }
            Color::Green => {
                if is_bg {
                    "\x1B[42m"
                } else {
                    "\x1B[32m"
                }
            }
            Color::Yellow => {
                if is_bg {
                    "\x1B[43m"
                } else {
                    "\x1B[33m"
                }
            }
            Color::Blue => {
                if is_bg {
                    "\x1B[44m"
                } else {
                    "\x1B[34m"
                }
            }
            Color::Magenta => {
                if is_bg {
                    "\x1B[45m"
                } else {
                    "\x1B[35m"
                }
            }
            Color::Cyan => {
                if is_bg {
                    "\x1B[46m"
                } else {
                    "\x1B[36m"
                }
            }
            Color::White => {
                if is_bg {
                    "\x1B[47m"
                } else {
                    "\x1B[37m"
                }
            }
            Color::BrightBlack => {
                if is_bg {
                    "\x1B[100m"
                } else {
                    "\x1B[90m"
                }
            }
            Color::BrightRed => {
                if is_bg {
                    "\x1B[101m"
                } else {
                    "\x1B[91m"
                }
            }
            Color::BrightGreen => {
                if is_bg {
                    "\x1B[102m"
                } else {
                    "\x1B[92m"
                }
            }
            Color::BrightYellow => {
                if is_bg {
                    "\x1B[103m"
                } else {
                    "\x1B[93m"
                }
            }
            Color::BrightBlue => {
                if is_bg {
                    "\x1B[104m"
                } else {
                    "\x1B[94m"
                }
            }
            Color::BrightMagenta => {
                if is_bg {
                    "\x1B[105m"
                } else {
                    "\x1B[95m"
                }
            }
            Color::BrightCyan => {
                if is_bg {
                    "\x1B[106m"
                } else {
                    "\x1B[96m"
                }
            }
            Color::BrightWhite => {
                if is_bg {
                    "\x1B[107m"
                } else {
                    "\x1B[97m"
                }
            }
        }
    }
}

pub trait ColorExt {
    fn color(self, color: Color) -> String;
    fn bg_color(self, color: Color) -> String;
    fn bold(self) -> String;
}

impl<T: Display> ColorExt for T {
    fn color(self, color: Color) -> String {
        format!(
            "{}{}{}",
            color.to_ansi_code(false),
            self,
            Color::Reset.to_ansi_code(false)
        )
    }

    fn bg_color(self, color: Color) -> String {
        format!(
            "{}{}{}",
            color.to_ansi_code(true),
            self,
            Color::Reset.to_ansi_code(true)
        )
    }

    fn bold(self) -> String {
        format!(
            "{}\x1B[1m{}{}",
            Color::Reset.to_ansi_code(false),
            self,
            Color::Reset.to_ansi_code(false)
        )
    }
}
