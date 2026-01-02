///
///  Handle Geogrid-Viewer binary overlay file format (.ovl)
///
///  Copyright (C) 2025 Ralf Horstmann <ralf@ackstorm.de>
///
///  This program is free software; you can redistribute it and/or modify
///  it under the terms of the GNU General Public License as published by
///  the Free Software Foundation; either version 2 of the License, or
///  (at your option) any later version.
///
///  This program is distributed in the hope that it will be useful,
///  but WITHOUT ANY WARRANTY; without even the implied warranty of
///  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
///  GNU General Public License for more details.
///
///  You should have received a copy of the GNU General Public License
///  along with this program; if not, write to the Free Software
///  Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
///
use anyhow::{anyhow, Result};
use core::sync::atomic::{AtomicU8, Ordering};

use nom::{
    branch::alt, bytes::complete::tag, bytes::complete::take, bytes::complete::take_till,
    error::context, number::complete::le_f64, number::complete::le_u16, number::complete::le_u32,
    Err, Parser,
};

use encoding_rs::mem::decode_latin1;

use crate::error::CustomError;
use crate::format::Format;
use crate::geodata::Geodata;
use crate::geodata::Waypoint;
use crate::geodata::WaypointList;

static DEBUG_LEVEL: AtomicU8 = AtomicU8::new(0);

fn get_debug() -> u8 {
    DEBUG_LEVEL.load(Ordering::Relaxed)
}

fn set_debug(debug: u8) {
    DEBUG_LEVEL.store(debug, Ordering::Relaxed);
}

fn ggv_bin_read_bytes<'a>(
    i: &'a [u8],
    len: u32,
    descr: &'static str,
) -> nom::IResult<&'a [u8], &'a [u8], CustomError> {
    let (i, res) = context(descr, take(len)).parse(i)?;
    Ok((i, res))
}

fn ggv_bin_read16<'a>(
    i: &'a [u8],
    descr: &'static str,
) -> nom::IResult<&'a [u8], u16, CustomError> {
    let (i, val) = context(descr, le_u16).parse(i)?;
    if get_debug() >= 2 {
        eprintln!("bin: {:<15} {:>5} (0x{:04x})", descr, val, val);
    }
    Ok((i, val))
}

fn ggv_bin_read32<'a>(
    i: &'a [u8],
    descr: &'static str,
) -> nom::IResult<&'a [u8], u32, CustomError> {
    let (i, val) = context(descr, le_u32).parse(i)?;
    if get_debug() >= 2 {
        if (val & 0xFFFF0000) == 0 {
            eprintln!("bin: {:<15} {:>5} (0x{:04x})", descr, val, val);
        } else {
            eprintln!("bin: {:<15} {:>5} (0x{:08x})", descr, val, val);
        }
    }
    Ok((i, val))
}

fn ggv_bin_read_text16<'a>(
    i: &'a [u8],
    descr: &'static str,
) -> nom::IResult<&'a [u8], String, CustomError> {
    let (i, len) = ggv_bin_read16(i, descr)?;
    let (i, buf) = ggv_bin_read_bytes(i, len.into(), descr)?;
    let (_, text) = context(descr, take_till(|c| c == b'\0')).parse(buf)?;
    let decoded: String = decode_latin1(text)
        .replace("\r\n", " ")
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|&c| !c.is_control())
        .collect();

    if get_debug() >= 2 {
        eprintln!("bin: {} = {:?}", descr, decoded);
    }
    Ok((i, decoded.to_owned()))
}

fn ggv_bin_read_text32<'a>(
    i: &'a [u8],
    descr: &'static str,
) -> nom::IResult<&'a [u8], String, CustomError> {
    let (i, len) = ggv_bin_read32(i, descr)?;
    // The following check prevents passing an unsigned int with a value
    // greater than INT32_MAX to a signed int parameter in
    // ggv_bin_read_bytes later on. Choosing a much lower limit of
    // UNIT16_MAX here since a larger value means the file is almost
    // certainly corrupted and some Qt versions throw std::bad_alloc
    // when getting close to INT32_MAX
    if len > u16::MAX.into() {
        eprintln!("bin: Read error, max len exceeded ({})", descr);
        let err = nom::Err::Failure(nom::error::make_error(i, nom::error::ErrorKind::TooLarge));
        return Err(err);
    }
    let (i, buf) = ggv_bin_read_bytes(i, len, descr)?;
    let (_, text) = context(descr, take_till(|c| c == b'\0')).parse(buf)?;
    let decoded: String = decode_latin1(text)
        .replace("\r\n", " ")
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|&c| !c.is_control())
        .collect();
    if get_debug() >= 2 {
        eprintln!("bin: {} = {:?}", descr, decoded);
    }
    Ok((i, decoded.to_owned()))
}

fn ggv_bin_parse_magic(buf: &[u8]) -> nom::IResult<&[u8], (u8, String), CustomError> {
    // example: "DOMGVCRD Ovlfile V3.0"
    let (_, magic) = context("magic", take(22usize)).parse(buf)?;
    let (buf, (_, m_version, _)) = context(
        "magic",
        (
            tag("DOMGVCRD Ovlfile V"),
            alt((tag("2"), tag("3"), tag("4"))),
            tag(".0:\0"),
        ),
    )
    .parse(buf)?;
    Ok((
        buf,
        ((m_version[0] - b'0'), decode_latin1(magic).into_owned()),
    ))
}

fn ggv_bin_read_double<'a>(
    i: &'a [u8],
    descr: &'static str,
) -> nom::IResult<&'a [u8], f64, CustomError> {
    let (i, val) = context(descr, le_f64).parse(i)?;
    Ok((i, val))
}

fn ggv_bin_write_bitmap<'a>(
    bitmap: &'a [u8],
    geodata: &mut Geodata,
) -> nom::IResult<&'a [u8], (), CustomError> {
    let (i, bmp_dib_size) = ggv_bin_read32(bitmap, "bmp dib size")?;
    if bmp_dib_size != 40 {
        return Ok((bitmap, ()));
    }
    let (i, _) = ggv_bin_read32(i, "bmp width")?;
    let (i, _) = ggv_bin_read32(i, "bmp height")?;
    let (i, _) = ggv_bin_read16(i, "bmp color plane")?;
    let (i, bmp_pixel_bits) = ggv_bin_read16(i, "bmp pixel bits")?;
    let (i, _) = ggv_bin_read32(i, "bmp compression")?;
    let (i, _) = ggv_bin_read32(i, "bmp image size")?;
    let (i, _) = ggv_bin_read32(i, "bmp x res")?;
    let (i, _) = ggv_bin_read32(i, "bmp y res")?;
    let (i, _) = ggv_bin_read32(i, "bmp num col")?;
    let (_, _) = ggv_bin_read32(i, "bmp imp col")?;
    let bmp_size: u32 = (bitmap.len() + 14) as u32;
    let bmp_reserved1: u16 = 0x00;
    let bmp_reserved2: u16 = 0x00;
    let bmp_offset: u32;
    // Files with 16bpp and above do not have a color table.
    if bmp_pixel_bits >= 16 {
        bmp_offset = 14 + bmp_dib_size;
    } else {
        bmp_offset = 14 + bmp_dib_size + 2u32.pow(bmp_pixel_bits.into()) * 4;
    }
    let mut data: Vec<u8> = Vec::new();
    data.append(&mut ("BM".as_bytes()).to_vec());
    data.append(&mut (bmp_size).to_le_bytes().to_vec());
    data.append(&mut (bmp_reserved1).to_le_bytes().to_vec());
    data.append(&mut (bmp_reserved2).to_le_bytes().to_vec());
    data.append(&mut (bmp_offset).to_le_bytes().to_vec());
    data.append(&mut bitmap.to_vec());
    geodata.add_data("bmp", data);
    Ok((bitmap, ()))
}

//////////////////////////////////////////////////////////////////////
//            OVL Version 2.0
//////////////////////////////////////////////////////////////////////

fn ggv_bin_read_v2_entries<'a>(
    buf: &'a [u8],
    entry_type: u16,
    track_name: &String,
    geodata: &mut Geodata,
) -> nom::IResult<&'a [u8], (), CustomError> {
    let mut buf = buf;
    match entry_type {
        2 => {
            let lat: f64;
            let lon: f64;
            let label: String;
            (buf, _) = ggv_bin_read16(buf, "text color")?;
            (buf, _) = ggv_bin_read16(buf, "text size")?;
            (buf, _) = ggv_bin_read16(buf, "text trans")?;
            (buf, _) = ggv_bin_read16(buf, "text font")?;
            (buf, _) = ggv_bin_read16(buf, "text angle")?;
            (buf, lon) = ggv_bin_read_double(buf, "text lon")?;
            (buf, lat) = ggv_bin_read_double(buf, "text lat")?;
            (buf, label) = ggv_bin_read_text16(buf, "text label")?;
            geodata.add_waypoint(
                Waypoint::new()
                    .with_lat(lat)
                    .with_lon(lon)
                    .with_name(&label),
            );
        }
        3 | 4 => {
            let mut waypoint_list = WaypointList::new();
            let line_points;
            let mut lat: f64;
            let mut lon: f64;
            (buf, _) = ggv_bin_read16(buf, "line color")?;
            (buf, _) = ggv_bin_read16(buf, "line width")?;
            (buf, _) = ggv_bin_read16(buf, "line type")?;
            (buf, line_points) = ggv_bin_read16(buf, "line points")?;
            if !track_name.is_empty() {
                waypoint_list.set_name(&track_name);
            }
            for _ in 1..=line_points {
                (buf, lon) = ggv_bin_read_double(buf, "text lon")?;
                (buf, lat) = ggv_bin_read_double(buf, "text lat")?;
                waypoint_list.add_waypoint(Waypoint::new().with_lat(lat).with_lon(lon));
            }
            geodata.add_track(waypoint_list);
        }
        5 | 6 | 7 => {
            (buf, _) = ggv_bin_read16(buf, "geom color")?;
            (buf, _) = ggv_bin_read16(buf, "geom prop1")?;
            (buf, _) = ggv_bin_read16(buf, "geom prop2")?;
            (buf, _) = ggv_bin_read16(buf, "geom angle")?;
            (buf, _) = ggv_bin_read16(buf, "geom stroke")?;
            (buf, _) = ggv_bin_read16(buf, "geom area")?;
            (buf, _) = ggv_bin_read_double(buf, "geom lon")?;
            (buf, _) = ggv_bin_read_double(buf, "geom lat")?;
        }
        9 => {
            let bmp_len;
            (buf, _) = ggv_bin_read16(buf, "bmp color")?;
            (buf, _) = ggv_bin_read16(buf, "bmp prop1")?;
            (buf, _) = ggv_bin_read16(buf, "bmp prop2")?;
            (buf, _) = ggv_bin_read16(buf, "bmp prop3")?;
            (buf, _) = ggv_bin_read_double(buf, "bmp lon")?;
            (buf, _) = ggv_bin_read_double(buf, "bmp lat")?;
            (buf, bmp_len) = ggv_bin_read32(buf, "bmp len")?;
            // The following check prevents passing an unsigned int with a value
            // greater than INT32_MAX to a signed int parameter in
            // ggv_bin_read_bytes later on. Choosing a much lower limit of
            // UNIT16_MAX here since a larger value means the file is almost
            // certainly corrupted and some Qt versions throw std::bad_alloc
            // when getting close to INT32_MAX
            if bmp_len > u16::MAX.into() {
                eprintln!("bin: Read error, max bmp_len exceeded");
                let err =
                    nom::Err::Failure(nom::error::make_error(buf, nom::error::ErrorKind::TooLarge));
                return Err(err);
            }
            let bmp_data;
            (buf, bmp_data) = ggv_bin_read_bytes(buf, bmp_len, "bmp data")?;
            let _ = ggv_bin_write_bitmap(bmp_data, geodata);
        }
        _ => {
            eprintln!("bin: Unsupported type: {:x}", entry_type);
            let err = nom::Err::Failure(nom::error::make_error(buf, nom::error::ErrorKind::Tag));
            return Err(err);
        }
    }
    Ok((buf, ()))
}

fn ggv_bin_read_header_v2(buf: &[u8]) -> nom::IResult<&[u8], String, CustomError> {
    let (buf, header_len) = ggv_bin_read16(buf, "map name len")?;
    if header_len > 0 {
        let (buf, _) = take(4usize)(buf)?;
        let (buf, name) = take(header_len - 4)(buf)?;
        let (_, name) = take_till(|c| c == b'\0')(name)?;
        let name = decode_latin1(name);
        if get_debug() >= 2 {
            eprintln!("bin: name = {:?}", name);
        }
        Ok((buf, name.into_owned()))
    } else {
        Ok((buf, String::new()))
    }
}

fn ggv_bin_read_v2<'a>(
    buf: &'a [u8],
    geodata: &mut Geodata,
) -> nom::IResult<&'a [u8], (), CustomError> {
    let mut buf = buf;
    let magic: String;
    let length = buf.len();
    (buf, (_, magic)) = ggv_bin_parse_magic(buf)?;
    if get_debug() >= 2 {
        eprintln!("bin: header = {}", magic);
    }
    (buf, _) = ggv_bin_read_header_v2(buf)?;
    while buf.len() > 0 {
        let pos = length - buf.len();
        let entry_type: u16;
        let entry_subtype: u16;
        if get_debug() >= 2 {
            eprintln!("------------------------------------ 0x{:x}", pos);
        }
        (buf, entry_type) = ggv_bin_read16(buf, "entry type")?;
        (buf, _) = ggv_bin_read16(buf, "entry group")?;
        (buf, _) = ggv_bin_read16(buf, "entry zoom")?;
        (buf, entry_subtype) = ggv_bin_read16(buf, "entry subtype")?;

        let mut track_name = String::new();
        if entry_subtype != 1 {
            let val: String;
            (buf, val) = ggv_bin_read_text32(buf, "track name")?;
            track_name = val;
        }
        (buf, _) = ggv_bin_read_v2_entries(buf, entry_type, &track_name, geodata)?;
    }
    Ok((buf, ()))
}

//////////////////////////////////////////////////////////////////////
//            OVL Version 3.0 and 4.0
//////////////////////////////////////////////////////////////////////

fn ggv_bin_read_header_v34(buf: &[u8]) -> nom::IResult<&[u8], (u32, u32), CustomError> {
    let mut buf = buf;
    let label_count;
    let record_count;
    let header_len;
    (buf, _) = ggv_bin_read_bytes(buf, 8, "unknown")?;
    (buf, label_count) = ggv_bin_read32(buf, "num labels")?;
    (buf, record_count) = ggv_bin_read32(buf, "num records")?;
    (buf, _) = ggv_bin_read_text16(buf, "text label")?;
    (buf, _) = ggv_bin_read16(buf, "unknown")?;
    (buf, _) = ggv_bin_read16(buf, "unknown")?;
    // 8 bytes ending with 1E 00, contains len of header block
    (buf, _) = ggv_bin_read16(buf, "unknown")?;
    (buf, header_len) = ggv_bin_read16(buf, "header len")?;
    (buf, _) = ggv_bin_read16(buf, "unknown")?;
    (buf, _) = ggv_bin_read16(buf, "unknown")?;
    if header_len > 0 {
        let mut map_name;
        (buf, map_name) = ggv_bin_read_bytes(buf, header_len.into(), "map name")?;
        (map_name, _) = take(4usize)(map_name)?;
        (_, map_name) = take_till(|c| c == b'\0')(map_name)?;
        if get_debug() >= 2 {
            eprintln!("bin: name = {:?}", decode_latin1(map_name));
        }
    }
    Ok((buf, (label_count, record_count)))
}

fn ggv_bin_read_label_v34(buf: &[u8], pos: usize) -> nom::IResult<&[u8], (), CustomError> {
    let mut buf = buf;
    if get_debug() >= 2 {
        eprintln!("------------------------------------ 0x{:x}", pos);
    }
    (buf, _) = ggv_bin_read_bytes(buf, 0x08, "label header")?;
    (buf, _) = ggv_bin_read_bytes(buf, 0x14, "label number")?;
    (buf, _) = ggv_bin_read_text16(buf, "label text")?;
    (buf, _) = ggv_bin_read16(buf, "label flag1")?;
    (buf, _) = ggv_bin_read16(buf, "label flag2")?;
    Ok((buf, ()))
}

fn ggv_bin_read_common_v34<'a>(buf: &'a [u8]) -> nom::IResult<&'a [u8], String, CustomError> {
    let mut buf = buf;
    let entry_text;
    let entry_type1;
    let entry_type2;
    (buf, _) = ggv_bin_read16(buf, "entry group")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop2")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop3")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop4")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop5")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop6")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop7")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop8")?;
    (buf, _) = ggv_bin_read16(buf, "entry zoom")?;
    (buf, _) = ggv_bin_read16(buf, "entry prop10")?;
    (buf, entry_text) = ggv_bin_read_text16(buf, "entry txt")?;
    (buf, entry_type1) = ggv_bin_read16(buf, "entry type1")?;
    if entry_type1 != 1 {
        (buf, _) = ggv_bin_read_text32(buf, "entry object")?;
    }
    (buf, entry_type2) = ggv_bin_read16(buf, "entry type2")?;
    if entry_type2 != 1 {
        (buf, _) = ggv_bin_read_text32(buf, "entry object")?;
    }
    Ok((buf, entry_text.to_owned()))
}

fn ggv_bin_read_record_v34<'a>(
    buf: &'a [u8],
    pos: usize,
    geodata: &mut Geodata,
) -> nom::IResult<&'a [u8], (), CustomError> {
    let mut buf = buf;
    let entry_type;
    let label;
    if get_debug() >= 2 {
        eprintln!("------------------------------------ 0x{:x}", pos);
    }
    (buf, entry_type) = ggv_bin_read16(buf, "entry type")?;
    (buf, label) = ggv_bin_read_common_v34(buf)?;

    match entry_type {
        0x02 => {
            // text
            let lat;
            let lon;
            let txt;
            (buf, _) = ggv_bin_read16(buf, "text prop1")?;
            (buf, _) = ggv_bin_read32(buf, "text prop2")?;
            (buf, _) = ggv_bin_read16(buf, "text prop3")?;
            (buf, _) = ggv_bin_read32(buf, "text prop4")?;
            (buf, _) = ggv_bin_read16(buf, "text ltype")?;
            (buf, _) = ggv_bin_read16(buf, "text angle")?;
            (buf, _) = ggv_bin_read16(buf, "text size")?;
            (buf, _) = ggv_bin_read16(buf, "text area")?;
            (buf, lon) = ggv_bin_read_double(buf, "text lon")?;
            (buf, lat) = ggv_bin_read_double(buf, "text lat")?;
            (buf, _) = ggv_bin_read_double(buf, "text unk")?;
            (buf, txt) = ggv_bin_read_text16(buf, "text label")?;
            geodata.add_waypoint(Waypoint::new().with_lat(lat).with_lon(lon).with_name(&txt));
        }
        //   area|line
        0x03 | 0x04 | 0x17 => {
            let line_points;
            (buf, _) = ggv_bin_read16(buf, "line prop1")?;
            (buf, _) = ggv_bin_read32(buf, "line prop2")?;
            (buf, _) = ggv_bin_read16(buf, "line prop3")?;
            (buf, _) = ggv_bin_read32(buf, "line color")?;
            (buf, _) = ggv_bin_read16(buf, "line size")?;
            (buf, _) = ggv_bin_read16(buf, "line stroke")?;
            (buf, line_points) = ggv_bin_read16(buf, "line points")?;

            if entry_type == 0x04 {
                // found in example.ovl generated by Geogrid-Viewer 1.0
                (buf, _) = ggv_bin_read16(buf, "line pad")?;
            }

            let mut track = WaypointList::new();
            if !label.is_empty() {
                track.set_name(&label);
            }
            for _ in 0..line_points {
                let lon;
                let lat;
                (buf, lon) = ggv_bin_read_double(buf, "line lon")?;
                (buf, lat) = ggv_bin_read_double(buf, "line lat")?;
                (buf, _) = ggv_bin_read_double(buf, "line unk")?;
                track.add_waypoint(Waypoint::new().with_lat(lat).with_lon(lon));
            }
            geodata.add_track(track);
        }
        0x05 | 0x06 | 0x07 => {
            (buf, _) = ggv_bin_read16(buf, "circle prop1")?;
            (buf, _) = ggv_bin_read32(buf, "circle prop2")?;
            (buf, _) = ggv_bin_read16(buf, "circle prop3")?;
            (buf, _) = ggv_bin_read32(buf, "circle color")?;
            (buf, _) = ggv_bin_read32(buf, "circle prop5")?;
            (buf, _) = ggv_bin_read32(buf, "circle prop6")?;
            (buf, _) = ggv_bin_read16(buf, "circle ltype")?;
            (buf, _) = ggv_bin_read16(buf, "circle angle")?;
            (buf, _) = ggv_bin_read16(buf, "circle size")?;
            (buf, _) = ggv_bin_read16(buf, "circle area")?;
            (buf, _) = ggv_bin_read_double(buf, "circle lon")?;
            (buf, _) = ggv_bin_read_double(buf, "circle lat")?;
            (buf, _) = ggv_bin_read_double(buf, "circle unk")?;
        }
        0x09 => {
            let bmp_len;
            (buf, _) = ggv_bin_read16(buf, "bmp prop1")?;
            (buf, _) = ggv_bin_read32(buf, "bmp prop2")?;
            (buf, _) = ggv_bin_read16(buf, "bmp prop3")?;
            (buf, _) = ggv_bin_read32(buf, "bmp prop4")?;
            (buf, _) = ggv_bin_read32(buf, "bmp prop5")?;
            (buf, _) = ggv_bin_read32(buf, "bmp prop6")?;
            (buf, _) = ggv_bin_read_double(buf, "bmp lon")?;
            (buf, _) = ggv_bin_read_double(buf, "bmp lat")?;
            (buf, _) = ggv_bin_read_double(buf, "bmp unk")?;
            (buf, bmp_len) = ggv_bin_read32(buf, "bmp len")?;
            // The following check prevents passing an unsigned int with a value
            // greater than INT32_MAX to a signed int parameter in
            // ggv_bin_read_bytes later on. Choosing a much lower limit of
            // UNIT16_MAX here since a larger value means the file is almost
            // certainly corrupted and some Qt versions throw std::bad_alloc
            // when getting close to INT32_MAX
            if bmp_len > u16::MAX.into() {
                eprintln!("bin: Read error, max bmp_len exceeded");
                let err =
                    nom::Err::Failure(nom::error::make_error(buf, nom::error::ErrorKind::TooLarge));
                return Err(err);
            }
            let bmp_data;
            (buf, _) = ggv_bin_read16(buf, "bmp prop")?;
            (buf, bmp_data) = ggv_bin_read_bytes(buf, bmp_len, "bmp data")?;
            let _ = ggv_bin_write_bitmap(bmp_data, geodata);
        }
        _ => {
            eprintln!("bin: Unsupported type: {:x}", entry_type);
            let err = nom::Err::Failure(nom::error::make_error(buf, nom::error::ErrorKind::Tag));
            return Err(err);
        }
    }

    Ok((buf, ()))
}

fn ggv_bin_read_v34<'a>(
    buf: &'a [u8],
    geodata: &mut Geodata,
) -> nom::IResult<&'a [u8], (), CustomError> {
    let mut buf = buf;
    let magic;
    let length = buf.len();
    (buf, (_, magic)) = ggv_bin_parse_magic(buf)?;
    if get_debug() >= 2 {
        eprintln!("bin: header = {}", magic);
    }
    while buf.len() > 0 {
        let label_count;
        let record_count;
        (buf, (label_count, record_count)) = ggv_bin_read_header_v34(buf)?;
        if label_count > 0 {
            if get_debug() >= 2 {
                eprintln!(
                    "-----labels------------------------- 0x{:x}",
                    length - buf.len()
                );
            }
            for _ in 0..label_count {
                (buf, _) = ggv_bin_read_label_v34(buf, length - buf.len())?;
            }
        }
        if record_count > 0 {
            if get_debug() >= 2 {
                eprintln!(
                    "-----records------------------------ 0x{:x}",
                    length - buf.len()
                );
            }
            for _ in 0..record_count {
                (buf, _) = ggv_bin_read_record_v34(buf, length - buf.len(), geodata)?;
            }
        }

        if buf.len() > 0 {
            if get_debug() >= 2 {
                eprintln!(
                    "------------------------------------ 0x{:x}",
                    length - buf.len()
                );
            }
            // we just skip over the next magic bytes without checking they
            // contain the correct string. This is consistent with what I
            // believe GGV does
            let magic;
            (buf, magic) = ggv_bin_read_bytes(buf, 23, "magicbytes")?;
            if get_debug() >= 2 {
                eprintln!("bin: header = {}", decode_latin1(magic));
            }
        }
    }
    Ok((buf, ()))
}

//////////////////////////////////////////////////////////////////////
//            entry points called by ggvtogpx main process
//////////////////////////////////////////////////////////////////////

pub struct GgvBinFormat {
    debug: u8,
}

impl Format for GgvBinFormat {
    fn probe(&self, buf: &[u8]) -> bool {
        match ggv_bin_parse_magic(buf) {
            Ok(_) => true,
            _ => false,
        }
    }
    fn read(&self, buf: &[u8]) -> Result<Geodata> {
        let mut geodata = Geodata::new().with_debug(self.debug);
        let ver = match ggv_bin_parse_magic(buf) {
            Ok((_, (v, _))) => v,
            _ => 0,
        };
        let result = match ver {
            2 => ggv_bin_read_v2(buf, &mut geodata),
            3 | 4 => ggv_bin_read_v34(buf, &mut geodata),
            _ => return Err(anyhow!("reading ggv_bin failed (undhandled version)")),
        };
        match result {
            Ok(_) => return Ok(geodata),
            Err(Err::Error(ref err)) => {
                return Err(anyhow!(format!(
                    "reading ggv_bin failed (version: {}, context: \"{}\")",
                    ver,
                    err.message()
                )));
            }
            Err(err) => {
                return Err(anyhow!(format!(
                    "reading ggv_bin failed (version: {}, context: \"{}\")",
                    ver, err
                )));
            }
        }
    }
    fn write(&self, _geodata: &Geodata) -> Result<String> {
        todo!("ggv_bin write support");
    }
    fn name<'a>(&self) -> &'a str {
        return "ggv_bin";
    }
    fn can_read(&self) -> bool {
        true
    }
    fn can_write(&self) -> bool {
        false
    }
    fn set_debug(&mut self, debug: u8) {
        set_debug(debug);
        self.debug = debug;
    }
}

impl GgvBinFormat {
    pub fn new() -> Self {
        set_debug(0);
        Self { debug: 0 }
    }
}
