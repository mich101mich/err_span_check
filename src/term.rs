use std::{
    io::{Result, Write},
    sync::{Mutex, MutexGuard},
};
use termcolor::{ColorChoice, ColorSpec, StandardStream as Stream, WriteColor};

static TERM: Mutex<Term> = Mutex::new(Term::new());

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum Colors {
    Normal,
    Red,
    Blue,
    Green,
    Yellow,

    Bold,
    BoldRed,
    BoldBlue,
    BoldGreen,
    BoldYellow,
}
impl Colors {
    pub(crate) fn is_bold(self) -> bool {
        use Colors::*;
        matches!(self, Bold | BoldRed | BoldBlue | BoldGreen | BoldYellow)
    }
    pub(crate) fn to_color(self) -> Option<termcolor::Color> {
        use Colors::*;
        match self {
            Red | BoldRed => Some(termcolor::Color::Red),
            Blue | BoldBlue => Some(termcolor::Color::Blue),
            Green | BoldGreen => Some(termcolor::Color::Green),
            Yellow | BoldYellow => Some(termcolor::Color::Yellow),
            Normal | Bold => None,
        }
    }
}

pub(crate) fn lock() -> MutexGuard<'static, Term> {
    TERM.lock().unwrap_or_else(|e| e.into_inner())
}

#[deny(unused_macros)]
macro_rules! print_col {
    ($col: ident => $($args:tt)+) => {{
        use std::io::Write;
        let mut term = $crate::term::lock();
        term.color = $crate::term::Colors::$col;
        let _ = std::write!(&mut *term, $($args)*);
        term.color = $crate::term::Colors::Normal;
    }};
    ($($args:tt)+) => { print_col!(Normal => $($args)+) };
}

#[deny(unused_macros)]
macro_rules! println_col {
    ($col:ident => $($args:tt)+) => {{
        use std::io::Write;
        let mut term = $crate::term::lock();
        term.color = $crate::term::Colors::$col;
        let _ = std::writeln!(&mut *term, $($args)*);
        term.color = $crate::term::Colors::Normal;
    }};
    ($($args:tt)+) => { println_col!(Normal => $($args)+) };
    () => { println_col!(Normal => "") };
}

pub(crate) struct Term {
    pub(crate) color: Colors,
    stream: Option<Stream>,
}

impl Term {
    const fn new() -> Self {
        Term {
            color: Colors::Normal,
            stream: None,
        }
    }
    fn stream(&mut self) -> &mut Stream {
        self.stream
            .get_or_insert_with(|| Stream::stderr(ColorChoice::Auto))
    }
}

impl Write for Term {
    // Color one line at a time because Travis does not preserve color setting
    // across output lines.
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if matches!(self.color, Colors::Normal) {
            return self.stream().write(buf);
        }

        let mut spec = ColorSpec::new();
        spec.set_fg(self.color.to_color());
        spec.set_bold(self.color.is_bold());

        let stream = self.stream();
        for line in buf.split_inclusive(|b| *b == b'\n') {
            let _ = stream.set_color(&spec);
            stream.write_all(line)?;
        }
        stream.reset()?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        self.stream().flush()
    }
}
