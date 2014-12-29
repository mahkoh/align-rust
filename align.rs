#![feature(phase)]
#[phase(plugin)]

extern crate regex_macros;
extern crate regex;
extern crate core;
extern crate getopts;

use std::io::{stdin, stderr, BufferedWriter};
use std::io::stdio::{stdout_raw};
use std::os::{args, set_exit_status};
use std::{uint, str};

use getopts::{getopts, optopt, optflag, usage};

use Alignment::{Left, Right, Centered};

struct DynVec<T> {
    vec: Vec<T>,
    default: T,
}

impl<T: Clone> DynVec<T> {
    fn new(default: T) -> DynVec<T> {
        DynVec { vec: Vec::new(), default: default }
    }

    fn get<'a>(&self, i: uint) -> T {
        if i < self.vec.len() {
            self.vec[i].clone()
        } else {
            self.default.clone()
        }
    }

    fn set(&mut self, index: uint, v: T) {
        let l = self.vec.len();
        if index >= l {
            self.vec.grow(index - l + 1u, self.default.clone());
        }
        self.vec[index] = v
    }

    fn push(&mut self, v: T) {
        self.default = v.clone();
        self.vec.push(v);
    }
}

#[deriving(Clone)]
enum Alignment {
    Left,
    Right,
    Centered,
}

struct Opts {
    str_delim: char,
    out_sep:   Vec<u8>,
    until:     uint,
    unicode:   bool,
    align:     DynVec<Alignment>,
    max_width: DynVec<uint>,
}

fn parse_opts() -> Result<Opts, ()> {
    let args = args();
    let prog_name = &args[0];

    let opts = [
        optopt("o", "", "set the output separator", "output separator"),
        optopt("s", "", "set the string delimiter", "string delimiter"),
        optopt("u", "", "set the maximum column", "until"),
        optflag("U", "", "read as UTF8 and use unicode width"),
        optflag("h", "", "print this help menu"),
    ];
    let matches = match getopts(args.tail(), &opts) {
        Ok(m) => m,
        Err(f) => {
            println!("{}", f);
            set_exit_status(1);
            return Err(());
        }
    };
    if matches.opt_present("h") {
        print!("{}", usage(prog_name.as_slice(), &opts));
        return Err(());
    }
    let unicode = matches.opt_present("U");
    let out_sep = match matches.opt_str("o") {
        Some(s) => s.into_bytes(),
        None => " ".to_string().into_bytes(),
    };
    let str_delim = match matches.opt_str("s") {
        Some(s) => {
            match s.len() {
                0 => 0u8 as char,
                _ => s.as_bytes()[0] as char,
            }
        },
        None => '"',
    };
    let until = match matches.opt_str("u") {
        Some(s) => match s.as_slice().parse() {
            Some(u) => u,
            None => {
                let _ = writeln!(&mut stderr(), "-u argument has to be a number");
                set_exit_status(1);
                return Err(());
            },
        },
        None => uint::MAX,
    };
    let mut align = DynVec::new(Left);
    let mut max_width = DynVec::new(0u);
    if matches.free.len() > 0 {
        let fmt = &matches.free[0];
        let test = regex!(r"(\d*)([<>=])");
        for c in test.captures_iter(fmt.as_slice()) {
            match c.at(1).unwrap() {
                p if p.len() > 0 => max_width.push(p.parse().unwrap()),
                _ => max_width.push(0),
            }
            match c.at(2).unwrap() {
                "<" => align.push(Left),
                ">" => align.push(Right),
                "=" => align.push(Centered),
                _ => unreachable!(),
            }
        }
        max_width.push(0);
    }
    Ok(Opts {
        str_delim: str_delim,
        out_sep: out_sep,
        until: until,
        unicode: unicode,
        align: align,
        max_width: max_width
    })
}

struct Words {
    line: Vec<u8>,
    words: Vec<(uint, uint)>,
}

impl Words {
    fn new(line: Vec<u8>, str_delim: char, until: uint) -> Words {
        let mut words = Vec::new();
        let mut pos = 0;
        loop {
            pos += match line.slice_from(pos).iter().position(|&c|
                                                         !(c as char).is_whitespace()) {
                Some(i) => i,
                None => break,
            };
            if words.len() == until {
                let end = match line.slice_from(pos).iter().position(|&c| c=='\n' as u8) {
                    Some(e) => pos + e,
                    None => line.len(),
                };
                words.push((pos, end));
                break;
            }
            let start = pos;
            let mut esc = false;
            let mut string = false;
            for (i, &c) in line.slice_from(start).iter().enumerate() {
                if !esc && c == str_delim as u8 {
                    string = !string;
                }
                esc = !esc && c == '\\' as u8;
                if c == '\n' as u8 || (!string && (c == ' ' as u8 || c == '\t' as u8)) {
                    pos += i;
                    break;
                }
            }
            words.push((start, pos));
        }
        Words { line: line, words: words }
    }

    fn iter<'a>(&'a self) -> WordIter<'a> {
        WordIter {
            pos: 0,
            line: &self.line,
            words: &self.words,
        }
    }
}

struct WordIter<'a> {
    pos: uint,
    line: &'a Vec<u8>,
    words: &'a Vec<(uint, uint)>,
}

impl<'a> Iterator<&'a [u8]> for WordIter<'a> {
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.pos < self.words.len() {
            let (start, end) = self.words[self.pos];
            self.pos += 1;
            Some(self.line.slice(start, end))
        } else {
            None
        }
    }
}

fn is_indent(c: u8) -> bool {
    c == ' ' as u8 || c == '\t' as u8
}

fn unsafe_byte_unicode_width(s: &[u8]) -> uint {
    unsafe { str::from_utf8_unchecked(s).width(false) }
}

fn read_as_bytes(opts: &mut Opts) -> (Option<Vec<u8>>, Vec<Words>) {
    let mut stdin = stdin();
    let mut indent: Option<Vec<u8>> = None;
    let mut lines = Vec::new();
    loop {
        let line = match stdin.read_until('\n' as u8) {
            Ok(l) => l,
            Err(..) => break,
        };
        if indent.is_none() {
            let tmp = line.iter().map(|c| *c).take_while(|c| is_indent(*c)).collect();
            indent = Some(tmp);
        }
        let line = Words::new(line, opts.str_delim, opts.until);
        for (i, word) in line.iter().enumerate() {
            if word.len() > opts.max_width.get(i) {
                opts.max_width.set(i, word.len());
            }
        }
        lines.push(line);
    }
    (indent, lines)
}

fn read_as_unicode(opts: &mut Opts) -> (Option<Vec<u8>>, Vec<Words>) {
    let mut stdin = stdin();
    let mut indent: Option<Vec<u8>> = None;
    let mut lines = Vec::new();
    loop {
        // Unicode validation up here
        let line = match stdin.read_line() {
            Ok(l) => l.into_bytes(),
            Err(..) => break,
        };
        if indent.is_none() {
            let tmp = line.iter().map(|c| *c).take_while(|c| is_indent(*c)).collect();
            indent = Some(tmp);
        }
        let line = Words::new(line, opts.str_delim, opts.until);
        for (i, word) in line.iter().enumerate() {
            let width = unsafe_byte_unicode_width(word);
            if width > opts.max_width.get(i) {
                opts.max_width.set(i, width);
            }
        }
        lines.push(line);
    }
    (indent, lines)
}

fn main() {
    let mut opts = match parse_opts() {
        Ok(o) => o,
        Err(..) => return,
    };

    let (indent, lines) = if opts.unicode {
        read_as_unicode(&mut opts)
    } else {
        read_as_bytes(&mut opts)
    };
    if lines.len() == 0 {
        return;
    }
    let indent = indent.unwrap();
    let padding = {
        let max_max_width = *opts.max_width.vec.iter().max().unwrap_or(&0);
        Vec::from_elem(max_max_width, ' ' as u8)
    };

    let mut stdout = BufferedWriter::new(stdout_raw());
    for line in lines.iter() {
        if line.words.len() > 0 {
            stdout.write(indent.as_slice()).unwrap();
        }
        let mut words = line.iter().enumerate().peekable();
        while let Some((i, word)) = words.next() {
            let pad = opts.max_width.get(i) - if opts.unicode {
                unsafe_byte_unicode_width(word)
            } else {
                word.len()
            };
            match opts.align.get(i) {
                Left => {
                    stdout.write(word).unwrap();
                    if words.peek().is_some() {
                        stdout.write(padding.slice_to(pad)).unwrap();
                    }
                },
                Right => {
                    stdout.write(padding.slice_to(pad)).unwrap();
                    stdout.write(word).unwrap();
                },
                Centered => {
                    stdout.write(padding.slice_to(pad/2)).unwrap();
                    stdout.write(word).unwrap();
                    if words.peek().is_some() {
                        stdout.write(padding.slice_to(pad-pad/2)).unwrap();
                    }
                },
            }
            if words.peek().is_some() {
                stdout.write(opts.out_sep.as_slice()).unwrap();
            }
        }
        stdout.write_str("\n").unwrap();
    }
}
