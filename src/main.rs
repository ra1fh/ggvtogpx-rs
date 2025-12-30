///
/// ggvtogpx main module
///
/// Copyright (C) 2025 Ralf Horstmann <ralf@ackstorm.de>
///
/// This program is free software; you can redistribute it and/or modify
/// it under the terms of the GNU General Public License as published by
/// the Free Software Foundation; either version 2 of the License, or
/// (at your option) any later version.
///
/// This program is distributed in the hope that it will be useful,
/// but WITHOUT ANY WARRANTY; without even the implied warranty of
/// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
/// GNU General Public License for more details.
///
/// You should have received a copy of the GNU General Public License
/// along with this program; if not, write to the Free Software
/// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
///
use std::env;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use clap::{Arg, Command};

mod error;
mod format;
mod geodata;
mod ggv_bin;
mod ggv_ovl;
mod ggv_ovl_tests;
mod ggv_xml;
mod gpx;

pub use crate::{error::*, format::*, geodata::*, ggv_bin::*, ggv_ovl::*, ggv_xml::*, gpx::*};

fn read_stdin() -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    io::stdin()
        .read_to_end(&mut buffer)
        .with_context(|| "couldn't read stdin")?;
    return Ok(buffer);
}

fn read_file(filename: &String) -> Result<Vec<u8>> {
    let path = Path::new(filename);
    let mut file = File::open(&path)
        .with_context(|| format!("couldn't open file for reading: {}", filename))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("couldn't read file: {}", filename))?;
    return Ok(buffer);
}

fn write_stdout(data: &String) -> Result<()> {
    io::stdout()
        .write_all(data.as_bytes())
        .with_context(|| "couldn't write stdout")?;
    Ok(())
}

fn write_file(data: &String, filename: &String) -> Result<()> {
    let mut out = std::fs::File::create(filename)
        .with_context(|| format!("failed to open file for writin: {}", filename))?;
    out.write_all(data.as_bytes())
        .with_context(|| format!("filed writing to file: {}", filename))?;
    Ok(())
}

fn main() -> Result<()> {
    let mut formats: Vec<Box<dyn Format>> = vec![
        Box::new(GgvBinFormat::new()),
        Box::new(GgvOvlFormat::new()),
        Box::new(GgvXmlFormat::new()),
    ];
    let format_names: Vec<&str> = formats.iter().map(|f| f.name()).collect();

    let matches = Command::new("ggvtogpx")
        .version("1.0")
        .about("Geogrid-Viewer to GPX Converter.")
        .arg(
            Arg::new("infile_p")
                .value_name("infile")
                .required(false)
                .help("input file (alternative to -f)"),
        )
        .arg(
            Arg::new("outfile_p")
                .value_name("outfile")
                .required(false)
                .help("output file (alternative to -F)"),
        )
        .arg(
            Arg::new("debug")
                .short('D')
                .value_parser(clap::value_parser!(u8).range(0..5))
                .help("debug <level> (0..5)"),
        )
        .arg(
            Arg::new("intype")
                .value_name("type")
                .short('i')
                .value_parser(format_names)
                .help("input <type>"),
        )
        .arg(
            Arg::new("infile")
                .value_name("file")
                .short('f')
                .help("input <file>"),
        )
        .arg(
            Arg::new("otype")
                .value_name("type")
                .short('o')
                .help("output <type> (ignored)"),
        )
        .arg(
            Arg::new("outfile")
                .value_name("file")
                .short('F')
                .help("output <file>"),
        )
        .get_matches();

    let debuglevel = *matches.get_one::<u8>("debug").unwrap_or(&0);
    formats.iter_mut().for_each(|f| f.set_debug(debuglevel));

    let infile = matches
        .get_one::<String>("infile")
        .or(matches.get_one::<String>("infile_p"));
    let indata = match infile {
        Some(p) => {
            if p == "-" {
                &read_stdin()?
            } else {
                &read_file(p)?
            }
        }
        None => &read_stdin()?,
    };

    let Some(format) = (match matches.get_one::<String>("intype") {
        Some(intype) => formats.iter().find(|&f| f.name() == intype),
        None => formats.iter().find(|&f| f.probe(indata)),
    }) else {
        return Err(anyhow!("input format not given or detected."));
    };
    if debuglevel >= 1 {
        eprintln!("main: using input format: {}", format.name());
    }

    let geodata = format.read(indata)?;

    let result = GpxFormat::new()
        .with_creator(&env::var("GGVTOGPX_CREATOR").unwrap_or("ggvtogpx".to_string()))
        .with_testmode(if env::var("GGVTOGPX_TESTMODE").is_ok() {
            true
        } else {
            false
        })
        .write(&geodata)?;

    let Some(outfile) = matches
        .get_one::<String>("outfile")
        .or(matches.get_one::<String>("outfile_p"))
    else {
        // Don't produce output without outfile option. Matches
        // gpsbabel behaviour and is useful for testing the input
        // code only.
        return Ok(());
    };

    if outfile == "-" {
        write_stdout(&result)?;
    } else {
        write_file(&result, outfile)?;
    }
    Ok(())
}
