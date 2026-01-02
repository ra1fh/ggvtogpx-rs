///
///  Support for "GeoGrid Viewer ascii overlay files".
///
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
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

use anyhow::{anyhow, Context, Result};
use encoding_rs::mem::decode_latin1;
use nom::{
    bytes::complete::tag, bytes::complete::take_while, character::complete::alphanumeric1,
    character::complete::multispace0, character::complete::space0, combinator::map,
    combinator::opt, error::context, error::Error, multi::many, sequence::delimited,
    sequence::pair, sequence::separated_pair, sequence::terminated, Err, IResult, Parser,
};

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

#[repr(u8)]
enum SymbolType {
    Bitmap = 1,
    Text = 2,
    Line = 3,
    Polygon = 4,
    Rectangle = 5,
    Circle = 6,
    Triangle = 7,
}

impl TryFrom<u8> for SymbolType {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(SymbolType::Bitmap),
            2 => Ok(SymbolType::Text),
            3 => Ok(SymbolType::Line),
            4 => Ok(SymbolType::Polygon),
            5 => Ok(SymbolType::Rectangle),
            6 => Ok(SymbolType::Circle),
            7 => Ok(SymbolType::Triangle),
            _ => Err(()),
        }
    }
}

impl fmt::Display for SymbolType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SymbolType::Bitmap => write!(f, "Bitmap"),
            SymbolType::Text => write!(f, "Text"),
            SymbolType::Line => write!(f, "Line"),
            SymbolType::Polygon => write!(f, "Polygon"),
            SymbolType::Rectangle => write!(f, "Rectangle"),
            SymbolType::Circle => write!(f, "Circle"),
            SymbolType::Triangle => write!(f, "Triangle"),
        }
    }
}

fn ggv_ovl_parse_section(i: &[u8]) -> IResult<&[u8], String, CustomError> {
    let (i, res) = context(
        "parse section",
        delimited(tag("["), take_while(|c| c != b']'), tag("]")),
    )
    .parse(i)?;
    Ok((i, decode_latin1(res).into_owned().trim().to_string()))
}

fn ggv_ovl_parse_key_value(i: &[u8]) -> IResult<&[u8], (String, String), CustomError> {
    let (i, key) = alphanumeric1(i)?;
    let (i, _) = (space0, tag("="), space0).parse(i)?;
    let (i, val) = take_while(|c| c != b'\n' && c != b';').parse(i)?;
    let (i, _) = opt(pair(tag(";"), take_while(|c| c != b'\n'))).parse(i)?;
    Ok((
        i,
        (
            decode_latin1(key).into_owned().trim().to_string(),
            decode_latin1(val).into_owned().trim().to_string(),
        ),
    ))
}

fn ggv_ovl_parse(
    i: &[u8],
) -> IResult<&[u8], HashMap<String, HashMap<String, String>>, CustomError> {
    map(
        many(
            0..,
            separated_pair(
                ggv_ovl_parse_section,
                multispace0,
                map(
                    context(
                        "key value",
                        many(0.., terminated(ggv_ovl_parse_key_value, multispace0)),
                    ),
                    |vec: Vec<_>| vec.into_iter().collect(),
                ),
            ),
        ),
        |vec: Vec<_>| vec.into_iter().collect(),
    )
    .parse(i)
}

fn ggv_ovl_process<'a>(ovl: &HashMap<String, HashMap<String, String>>) -> Result<Geodata> {
    let mut geodata = Geodata::new().with_debug(get_debug());
    let mut route_count = 1;
    let mut track_count = 1;
    let mut waypoint_count = 1;
    let overlay = ovl.get("Overlay").with_context(|| "Overlay missing")?;
    let symbols = overlay.get("Symbols").with_context(|| "Symbols missing")?;
    let symbols: u16 = symbols.parse().with_context(|| "Symbols u16")?;
    if get_debug() >= 2 {
        eprintln!("ovl: Symbols: {}", symbols)
    };
    for i in 1..=symbols {
        let key = format!("Symbol {}", i);
        let symbol = ovl.get(&key).with_context(|| format!("{} missing", key))?;
        if get_debug() >= 2 {
            eprintln!("ovl: === {} ===", key)
        };
        let typ_str = symbol.get("Typ").with_context(|| format!("{}, Typ", key))?;
        let typ_int: u8 = typ_str
            .parse()
            .with_context(|| format!("{}, Typ int", key))?;
        let typ: SymbolType =
            SymbolType::try_from(typ_int).map_err(|_| anyhow!(format!("{}, Typ enum", key)))?;
        if get_debug() >= 2 {
            eprintln!("ovl: type: {} ({})", typ, typ_int)
        };
        match typ {
            SymbolType::Line | SymbolType::Polygon => {
                let mut waypoint_list = WaypointList::new();
                let group = symbol
                    .get("Group")
                    .with_context(|| format!("{}, Group", key))?;
                let group: u16 = group
                    .parse()
                    .with_context(|| format!("{}, Group u16", key))?;
                if get_debug() >= 2 {
                    eprintln!("ovl: Group: {}", group)
                };
                let points = symbol
                    .get("Punkte")
                    .with_context(|| format!("{}, Punkte", key))?;
                let points: u16 = points
                    .parse()
                    .with_context(|| format!("{}, Punkte u16", key))?;
                if get_debug() >= 2 {
                    eprintln!("ovl: Punkte: {}", points)
                };
                for j in 0..points {
                    let ykoord = symbol
                        .get(&format!("YKoord{}", j))
                        .with_context(|| format!("{}, YKoord{}", key, j))?;
                    let ykoord: f64 = ykoord
                        .parse::<f64>()
                        .with_context(|| format!("{}, YKoord{} f64", key, j))?;
                    let xkoord = symbol
                        .get(&format!("XKoord{}", j))
                        .with_context(|| format!("{}, XKoord{}", key, j))?;
                    let xkoord: f64 = xkoord
                        .parse::<f64>()
                        .with_context(|| format!("{}, XKoord{} f64", key, j))?;
                    let mut waypoint = Waypoint::new().with_lat(ykoord).with_lon(xkoord);
                    if group > 1 {
                        waypoint.set_name(&format!("RPT{:03}", waypoint_count));
                        waypoint_count += 1;
                    }
                    waypoint_list.add_waypoint(waypoint);
                    if get_debug() >= 3 {
                        eprintln!(
                            "ovl: YKoord/Lat: {:09.5}, XKoord/Lon: {:08.5}",
                            ykoord, xkoord
                        )
                    }
                }
                match symbol.get("Text") {
                    Some(text) => {
                        waypoint_list.set_name(text);
                    }
                    None => {
                        if group > 1 {
                            waypoint_list.set_name(&format!("Route {}", route_count));
                            route_count += 1;
                        } else {
                            waypoint_list.set_name(&format!("Track {}", track_count));
                            track_count += 1;
                        }
                    }
                }
                if group > 1 {
                    geodata.add_route(waypoint_list);
                } else {
                    geodata.add_track(waypoint_list);
                }
            }
            SymbolType::Text
            | SymbolType::Rectangle
            | SymbolType::Circle
            | SymbolType::Triangle => {
                let ykoord = symbol
                    .get("YKoord")
                    .with_context(|| format!("{}, YKoord", key))?;
                let ykoord: f64 = ykoord
                    .parse::<f64>()
                    .with_context(|| format!("{}, YKoord f64", key))?;
                let xkoord = symbol
                    .get("XKoord")
                    .with_context(|| format!("{}, XKoord", key))?;
                let xkoord: f64 = xkoord
                    .parse::<f64>()
                    .with_context(|| format!("{}, XKoord f64", key))?;
                if get_debug() >= 3 {
                    eprintln!(
                        "ovl: YKoord/Lat: {:09.5}, XKoord/Lon: {:08.5}",
                        ykoord, xkoord
                    )
                }
                let mut waypoint = Waypoint::new().with_lat(ykoord).with_lon(xkoord);
                match symbol.get("Text") {
                    Some(text) => {
                        waypoint.set_name(text);
                    }
                    None => {
                        waypoint.set_name(&key);
                    }
                }
                geodata.add_waypoint(waypoint);
            }
            SymbolType::Bitmap => {}
        }
    }
    Ok(geodata)
}

//////////////////////////////////////////////////////////////////////
//            entry points called by ggvtogpx main process
//////////////////////////////////////////////////////////////////////

pub struct GgvOvlFormat {
    debug: u8,
}

impl Format for GgvOvlFormat {
    fn probe(&self, buf: &[u8]) -> bool {
        if tag::<_, _, Error<_>>("[Symbol").parse(buf).is_ok()
            || tag::<_, _, Error<_>>("[Overlay").parse(buf).is_ok()
        {
            return true;
        } else {
            return false;
        }
    }
    fn read(&self, buf: &[u8]) -> Result<Geodata> {
        let ovl = match ggv_ovl_parse(buf) {
            Ok((_, res)) => res,
            Err(Err::Error(ref err)) => {
                return Err(anyhow!(format!(
                    "reading ggv_ovl failed (function: parse, context: \"{}\")",
                    err.message()
                )));
            }
            Err(err) => {
                return Err(anyhow!(format!(
                    "reading ggv_ovl failed (function: parse, context: \"{}\")",
                    err
                )));
            }
        };
        if self.debug >= 3 {
            eprintln!("ovl: input size: {}", buf.len());
        }
        let geodata = match ggv_ovl_process(&ovl) {
            Ok(g) => g,
            Err(err) => {
                return Err(anyhow!(
                    "reading ggv_ovl failed (function: process, context: \"{}\")",
                    err
                ))
            }
        };
        Ok(geodata)
    }
    fn write(&self, geodata: &Geodata) -> Result<String> {
        let mut result: Vec<String> = Vec::new();
        Ok(result.join("\r\n") + "\r\n")
    }
    fn name<'a>(&self) -> &'a str {
        return "ggv_ovl";
    }
    fn can_read(&self) -> bool {
        true
    }
    fn can_write(&self) -> bool {
        true
    }
    fn set_debug(&mut self, debug: u8) {
        set_debug(debug);
        self.debug = debug;
    }
}

impl GgvOvlFormat {
    pub fn new() -> Self {
        Self { debug: 0 }
    }
}
