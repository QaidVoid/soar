use libc::{ioctl, winsize, STDOUT_FILENO, TIOCGWINSZ};
use std::mem;

pub struct Grid {
    cols: usize,
    col_widths: Vec<Option<usize>>,
    col_ratios: Vec<f64>,
    rows: Vec<Vec<String>>,
    separator: String,
}

impl Grid {
    pub fn builder(cols: usize) -> GridBuilder {
        GridBuilder {
            cols,
            col_widths: vec![None; cols],
            col_ratios: vec![1.0; cols],
            separator: String::new(),
        }
    }

    fn get_terminal_width() -> usize {
        let mut w: winsize = unsafe { mem::zeroed() };

        if unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut w) } == 0 {
            if w.ws_col > 0 {
                w.ws_col as usize
            } else {
                80
            }
        } else {
            80
        }
    }

    fn calculate_column_widths(&mut self) {
        let mut total_fixed_width = 0;
        let mut total_ratio = 0.0;

        for (i, &width) in self.col_widths.iter().enumerate() {
            if let Some(w) = width {
                total_fixed_width += w;
            } else {
                total_ratio += self.col_ratios[i];
            }
        }

        let terminal_width = Grid::get_terminal_width();

        if total_fixed_width >= terminal_width {
            panic!("Total fixed column widths exceed the terminal width");
        }

        let remaining_width = terminal_width.saturating_sub(total_fixed_width);

        for (i, width) in self.col_widths.iter_mut().enumerate() {
            if width.is_none() {
                let ratio = self.col_ratios[i];
                *width = Some((remaining_width as f64 * (ratio / total_ratio)).round() as usize);
            }
        }
    }

    pub fn row(&mut self, row: Vec<String>) -> &mut Self {
        if row.len() != self.cols {
            panic!("Row length must match the number of columns.");
        }
        self.rows.push(row);
        self
    }

    fn truncate_row(row: &[String], widths: &[usize]) -> Vec<String> {
        row.iter()
            .enumerate()
            .map(|(i, col)| {
                let max_width = widths[i];
                if col.len() > max_width {
                    format!("{}...", &col[..max_width.saturating_sub(3)])
                } else {
                    col.clone()
                }
            })
            .collect()
    }

    pub fn print(mut self) {
        self.calculate_column_widths();

        let column_widths: Vec<usize> = self.col_widths.iter().map(|w| w.unwrap_or(0)).collect();

        for row in &self.rows {
            let truncated_row = Grid::truncate_row(row, &column_widths);
            for (i, col) in truncated_row.iter().enumerate() {
                let width = column_widths[i];
                print!("{:width$} ", col, width = width);
                if i < self.cols - 1 {
                    print!("{}", self.separator)
                }
            }
            println!();
        }
    }
}

pub struct GridBuilder {
    cols: usize,
    col_widths: Vec<Option<usize>>,
    col_ratios: Vec<f64>,
    separator: String,
}

impl GridBuilder {
    pub fn set_width(mut self, col: usize, width: usize) -> Self {
        if col < self.cols {
            self.col_widths[col] = Some(width);
        }
        self
    }

    pub fn set_ratio(mut self, col: usize, ratio: f64) -> Self {
        if col < self.cols {
            self.col_ratios[col] = ratio;
        }
        self
    }

    pub fn set_separator(mut self, separator: String) -> Self {
        self.separator = separator;
        self
    }

    pub fn build(self) -> Grid {
        Grid {
            cols: self.cols,
            col_widths: self.col_widths,
            col_ratios: self.col_ratios,
            rows: Vec::new(),
            separator: self.separator,
        }
    }
}
