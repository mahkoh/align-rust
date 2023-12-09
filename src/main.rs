#![allow(clippy::len_zero, clippy::manual_range_contains)]

use {
    anyhow::{bail, Context, Error},
    clap::{
        builder::{styling::AnsiColor, Styles},
        Parser,
    },
    std::io::{stdin, stdout, Write},
    unicode_width::UnicodeWidthStr,
};

use Alignment::{Centered, Left, Right};

#[derive(Clone)]
struct DynVec<T> {
    vec: Vec<T>,
    default: T,
}

impl<T: Copy + Clone> DynVec<T> {
    fn new(default: T) -> DynVec<T> {
        DynVec {
            vec: Vec::new(),
            default,
        }
    }

    fn get(&self, i: usize) -> T {
        if i < self.vec.len() {
            self.vec[i]
        } else {
            self.default
        }
    }

    fn set(&mut self, index: usize, v: T) {
        let l = self.vec.len();
        if index >= l {
            self.vec.resize(index + 1, self.default);
        }
        self.vec[index] = v
    }

    fn push(&mut self, v: T) {
        self.default = v;
        self.vec.push(v);
    }
}

#[derive(Copy, Clone)]
enum Alignment {
    Left,
    Right,
    Centered,
}

#[derive(Clone)]
struct Positioning {
    max_width: DynVec<usize>,
    align: DynVec<Alignment>,
}

impl Default for Positioning {
    fn default() -> Self {
        Self {
            max_width: DynVec::new(0),
            align: DynVec::new(Left),
        }
    }
}

fn parse_positioning(mut fmt: &str) -> Result<Positioning, Error> {
    let mut align = DynVec::new(Left);
    let mut max_width = DynVec::new(0);
    while fmt.len() > 0 {
        let non_digit = match fmt.as_bytes().iter().position(|&c| c < b'0' || c > b'9') {
            Some(i) => i,
            _ => bail!("Invalid format sequence"),
        };
        if non_digit > 0 {
            max_width.push(
                fmt[..non_digit]
                    .parse()
                    .with_context(|| format!("Invalid width {}", &fmt[..non_digit]))?,
            );
        } else {
            max_width.push(0);
        }
        match fmt.as_bytes()[non_digit] {
            b'<' => align.push(Left),
            b'>' => align.push(Right),
            b'=' => align.push(Centered),
            c => bail!("Invalid format character: {}", c as char),
        }
        fmt = &fmt[non_digit + 1..];
    }
    max_width.push(0);
    Ok(Positioning { max_width, align })
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Yellow.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
}

/// Reads text from stdin, aligns columns, and prints the result to stdout.
#[derive(Parser)]
#[command(styles = styles())]
struct Opts {
    /// The output separator.
    ///
    /// By default, columns are separated by a space.
    #[arg(short = 'o', value_name = "output separator", default_value_t = {" ".to_string()}, hide_default_value = true)]
    out_sep: String,
    /// The string delimiter.
    ///
    /// By default, strings are delimited by `"`.
    #[arg(
        short = 's',
        value_name = "string delimiter",
        default_value_t = '"',
        hide_default_value = true
    )]
    str_delim: char,
    /// The maximum column.
    ///
    /// By default, there is no maximum.
    #[arg(short = 'u', value_name = "until", default_value_t = usize::MAX, hide_default_value = true)]
    until: usize,
    /// The positioning of the columns.
    ///
    /// By default, all columns are left aligned.
    ///
    /// Example: <50>=<{n}
    /// {n}- The first column is left aligned
    /// {n}- The second column is right aligned and has a minimum width of 50
    /// {n}- The third column is centered
    /// {n}- The fourth and all following columns are left aligned
    #[arg(value_parser = parse_positioning, default_value = "", hide_default_value = true)]
    positioning: Positioning,
}

struct Words {
    line: String,
    words: Vec<(usize, usize)>,
}

impl Words {
    fn new(line: String, str_delim: char, until: usize) -> Words {
        let mut words = Vec::new();
        let mut pos = 0;
        while pos < line.len() {
            let line = line.as_bytes();
            pos += match line[pos..]
                .iter()
                .position(|&c| !(c as char).is_whitespace())
            {
                Some(i) => i,
                None => break,
            };
            if words.len() == until {
                words.push((pos, line.len()));
                break;
            }
            let start = pos;
            let mut end = line.len();
            let mut esc = false;
            let mut string = false;
            for (i, &c) in line[start..].iter().enumerate() {
                if !esc && c == str_delim as u8 {
                    string = !string;
                }
                esc = !esc && c == b'\\';
                if !string && (c == b' ' || c == b'\t') {
                    end = start + i;
                    break;
                }
            }
            pos = end;
            words.push((start, end));
        }
        Words { line, words }
    }

    fn iter(&self) -> WordIter<'_> {
        WordIter {
            pos: 0,
            line: &self.line,
            words: &self.words,
        }
    }
}

struct WordIter<'a> {
    pos: usize,
    line: &'a String,
    words: &'a Vec<(usize, usize)>,
}

impl<'a> Iterator for WordIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.pos < self.words.len() {
            let (start, end) = self.words[self.pos];
            self.pos += 1;
            Some(&self.line[start..end])
        } else {
            None
        }
    }
}

fn is_indent(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

fn read_as_unicode(opts: &mut Opts) -> (Option<Vec<u8>>, Vec<Words>) {
    let stdin = stdin();
    let mut indent: Option<Vec<u8>> = None;
    let mut lines = Vec::new();
    for line in stdin.lines() {
        let Ok(line) = line else { break };
        if indent.is_none() {
            let tmp = line
                .as_bytes()
                .iter()
                .copied()
                .take_while(|c| is_indent(*c))
                .collect();
            indent = Some(tmp);
        }
        let line = Words::new(line, opts.str_delim, opts.until);
        for (i, word) in line.iter().enumerate() {
            let width = word.width();
            if width > opts.positioning.max_width.get(i) {
                opts.positioning.max_width.set(i, width);
            }
        }
        lines.push(line);
    }
    (indent, lines)
}

fn main() {
    let mut opts = Opts::parse();

    let (indent, lines) = read_as_unicode(&mut opts);
    if lines.len() == 0 {
        return;
    }
    let indent = indent.unwrap();
    let padding = {
        let max_max_width = *opts.positioning.max_width.vec.iter().max().unwrap_or(&0);
        vec![b' '; max_max_width]
    };

    let mut stdout = stdout().lock();
    for line in lines.iter() {
        if line.words.len() > 0 {
            stdout.write_all(&indent).unwrap();
        }
        let mut words = line.iter().enumerate().peekable();
        while let Some((i, word)) = words.next() {
            let pad = opts.positioning.max_width.get(i) - word.width();
            match opts.positioning.align.get(i) {
                Left => {
                    stdout.write_all(word.as_bytes()).unwrap();
                    if words.peek().is_some() {
                        stdout.write_all(&padding[0..pad]).unwrap();
                    }
                }
                Right => {
                    stdout.write_all(&padding[0..pad]).unwrap();
                    stdout.write_all(word.as_bytes()).unwrap();
                }
                Centered => {
                    stdout.write_all(&padding[0..pad / 2]).unwrap();
                    stdout.write_all(word.as_bytes()).unwrap();
                    if words.peek().is_some() {
                        stdout.write_all(&padding[0..pad - pad / 2]).unwrap();
                    }
                }
            }
            if words.peek().is_some() {
                stdout.write_all(opts.out_sep.as_bytes()).unwrap();
            }
        }
        stdout.write_all(b"\n").unwrap();
    }
}
