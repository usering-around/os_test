// Copyright (c) 2020 hxdmp developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

// modified from https://github.com/rustyhorde/hxdmp/blob/master/src/lib.rs/
use core::fmt::{self, Write};

type Result<T> = core::result::Result<T, fmt::Error>;

pub fn hexdumpm<W>(buffer: &[u8], max_lines_opt: Option<usize>, writer: &mut W) -> Result<()>
where
    W: Write,
{
    let sixteen_iter = buffer.chunks(16).enumerate();

    if let Some(max) = max_lines_opt {
        for (line, parts) in sixteen_iter {
            if line < max {
                hex(line, parts, writer)?;
            } else {
                break;
            }
        }
    } else {
        for (line, parts) in sixteen_iter {
            hex(line, parts, writer)?;
        }
    }
    Ok(())
}

fn hex<W>(line: usize, parts: &[u8], writer: &mut W) -> Result<()>
where
    W: Write,
{
    if line > 0 {
        writeln!(writer)?;
    }
    write!(writer, "{:04}: ", line * 16)?;
    for b in parts {
        write!(writer, "{:02X} ", b)?;
    }

    for _ in parts.len()..16 {
        write!(writer, "   ")?;
    }

    write!(writer, " ")?;

    for b in parts {
        let ch = *b as char;
        if ch.is_ascii_graphic() {
            write!(writer, "{}", ch)?;
        } else {
            write!(writer, ".")?;
        }
    }
    Ok(())
}
