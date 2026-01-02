///
///  Support for "GeoGrid Viewer XML overlay files".
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
use std::io::BufReader;
use std::io::Read;

use anyhow::{anyhow, Context, Result};
use core::sync::atomic::{AtomicU8, Ordering};
use encoding_rs::mem::decode_latin1;
use encoding_rs::mem::encode_latin1_lossy;

use nom::{bytes::complete::tag, error::Error, Parser};

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

/// Parse single coordinate data
fn ggv_xml_parse_coord(coord: &roxmltree::Node) -> Option<Waypoint> {
    let x_coord = coord.attribute("x")?;
    let x_double = x_coord.parse::<f64>().ok()?;
    let y_coord = coord.attribute("y")?;
    let y_double = y_coord.parse::<f64>().ok()?;
    let z_double: Option<f64> = coord
        .attribute("z")
        .filter(|v| *v != "-32768")
        .and_then(|v| v.parse::<f64>().ok());
    match z_double {
        Some(z_double) => Some(
            Waypoint::new()
                .with_lat(y_double)
                .with_lon(x_double)
                .with_elevation(z_double),
        ),
        None => Some(Waypoint::new().with_lat(y_double).with_lon(x_double)),
    }
}

/// Parse attributeList that contains the actuall coordinates
fn ggv_xml_parse_attributelist(attribute_list: &roxmltree::Node) -> Option<WaypointList> {
    let mut waypoint_list = WaypointList::new();
    for attribute_tag in attribute_list
        .children()
        .filter(|c| c.has_tag_name("attribute"))
    {
        let iid_name = attribute_tag.attribute("iidName").unwrap_or("");
        if get_debug() >= 2 {
            eprintln!("xml: iidName: {}", iid_name);
        };
        if iid_name == "IID_IGraphicTextAttributes" {
            let Some(text_attr) = attribute_tag.children().find(|c| c.has_tag_name("text")) else {
                continue;
            };
            let Some(text_val) = text_attr.text() else {
                continue;
            };
            waypoint_list.set_name(text_val);
            if get_debug() >= 2 {
                eprintln!("xml: text: {}", waypoint_list.name());
            }
        } else if iid_name == "IID_IGraphic" {
            let Some(coord_list) = attribute_tag
                .children()
                .find(|c| c.has_tag_name("coordList"))
            else {
                continue;
            };
            for coord in coord_list.children().filter(|c| c.has_tag_name("coord")) {
                let Some(waypoint) = ggv_xml_parse_coord(&coord) else {
                    continue;
                };
                if get_debug() >= 3 {
                    eprintln!(
                        "xml: coord: {:09.5} {:08.5} {:.1}",
                        waypoint.latitude(),
                        waypoint.longitude(),
                        waypoint.elevation()
                    );
                }
                waypoint_list.add_waypoint(waypoint);
            }
        }
    }
    if waypoint_list.waypoints().len() > 0 {
        Some(waypoint_list)
    } else {
        None
    }
}

/// Parse name out of <base><name>...</name></base>
fn ggv_xml_read_name(object: &roxmltree::Node) -> Option<String> {
    let base = object.children().find(|c| c.has_tag_name("base"))?;
    let name_element = base.children().find(|c| c.has_tag_name("name"))?;
    let text_plain = name_element.text()?;
    // The xml is supposed to be encoded in latin1. Sometimes it still
    // has UTF-8 encoded names. Account for that by trying to convert
    // names back to bytes and attempt UTF-8 conversion.
    let text_utf8 = String::from_utf8(encode_latin1_lossy(text_plain).to_vec());
    match text_utf8 {
        Ok(text) => Some(text.to_string()),
        _ => Some(text_plain.to_string()),
    }
}

/// Parse object elements from objectList
fn ggv_xml_read_object(object: &roxmltree::Node, geodata: &mut Geodata) {
    let cls_name = object.attribute("clsName").unwrap_or("");
    let uid = object.attribute("uid").unwrap_or("");

    if get_debug() >= 2 {
        eprintln!("xml: === clsName: {:?} ===", cls_name);
        eprintln!("xml: uid: {:?}", uid);
    }

    if cls_name != "CLSID_GraphicLine"
        && cls_name != "CLSID_GraphicCircle"
        && cls_name != "CLSID_GraphicText"
    {
        return;
    }

    let name = ggv_xml_read_name(object).unwrap_or(String::from(""));
    if get_debug() >= 2 {
        eprintln!("xml: name: {}", name);
    }

    let Some(attribute_list) = object.children().find(|c| c.has_tag_name("attributeList")) else {
        return;
    };

    let Some(mut waypoint_list) = ggv_xml_parse_attributelist(&attribute_list) else {
        return;
    };

    if get_debug() >= 2 {
        eprintln!(
            "xml: waypoint_list len: {}",
            waypoint_list.waypoints().len()
        );
    }

    if cls_name == "CLSID_GraphicLine" {
        if name.is_empty() || name == "Teilstrecke" || name == "Line" {
            let number_tracks = geodata.tracks().len();
            waypoint_list.set_name(&format!("Track {:03}", number_tracks + 1));
        } else {
            waypoint_list.set_name(&name);
        }
        geodata.add_track(waypoint_list);
    } else if cls_name == "CLSID_GraphicCircle" {
        let mut waypoint = waypoint_list.extract_first_waypoint().clone();
        if name.is_empty() || name == "Circle" {
            waypoint.set_name(&format!("RPT{:03}", geodata.waypoints_len() + 1));
        } else {
            waypoint.set_name(&name);
        }
        geodata.add_waypoint(waypoint);
    } else if cls_name == "CLSID_GraphicText" {
        let mut waypoint = waypoint_list.extract_first_waypoint().clone();
        if waypoint_list.name().is_empty() || waypoint_list.name() == "Text" {
            waypoint.set_name(&format!("Text {}", geodata.waypoints_len() + 1));
        } else {
            waypoint.set_name(&waypoint_list.name());
        }
        geodata.add_waypoint(waypoint);
    }
}

/// Parse objectList elements
fn ggv_xml_read_object_list(object_list: roxmltree::Node, geodata: &mut Geodata) {
    for object in object_list.children().filter(|c| c.has_tag_name("object")) {
        ggv_xml_read_object(&object, geodata);
    }
}

/// Parse geogrid50.xml
fn ggv_xml_process_xml<'a>(xml: &str) -> Result<Geodata> {
    let mut geodata = Geodata::new().with_debug(get_debug());
    let doc = roxmltree::Document::parse(xml).with_context(|| "parse xml")?;
    let root = doc.root().first_child().with_context(|| "root node")?;
    root.has_tag_name("geogridOvl")
        .then_some(())
        .ok_or_else(|| anyhow!("geogridOvl tag"))?;
    for object_list in root.children().filter(|c| c.has_tag_name("objectList")) {
        ggv_xml_read_object_list(object_list, &mut geodata);
    }
    Ok(geodata)
}

/// Extract geogrid50.xml from zip
fn ggv_xml_extract_zip<'a>(i: &'a [u8]) -> Result<String> {
    let mut buf_reader = BufReader::new(i);
    loop {
        match zip::read::read_zipfile_from_stream(&mut buf_reader) {
            Ok(Some(mut file)) => {
                if file.name() == "geogrid50.xml" {
                    if get_debug() >= 2 {
                        eprintln!("xml: found geogrid50.xml");
                    }
                    let mut xml_buf = Vec::new();
                    file.read_to_end(&mut xml_buf)
                        .with_context(|| "reading geogrid50.xml from zip")?;
                    let xml_str = decode_latin1(&xml_buf).to_string();
                    return Ok(xml_str);
                }
            }
            Ok(None) => break,
            Err(e) => return Err(anyhow!(e)),
        }
    }
    Err(anyhow!("finding geogrid50.xml in zip"))
}

//////////////////////////////////////////////////////////////////////
//            entry points called by ggvtogpx main process
//////////////////////////////////////////////////////////////////////

pub struct GgvXmlFormat {
    debug: u8,
}

impl Format for GgvXmlFormat {
    fn probe(&self, buf: &[u8]) -> bool {
        if tag::<_, _, Error<_>>("PK\x03\x04").parse(buf).is_ok() {
            return true;
        } else {
            return false;
        }
    }
    fn read(&self, buf: &[u8]) -> Result<Geodata> {
        if self.debug >= 3 {
            eprintln!("xml: input size: {}", buf.len());
        }
        let xml = match ggv_xml_extract_zip(buf) {
            Ok(d) => d,
            Err(e) => {
                return Err(anyhow!(
                    "reading ggv_xml failed (extract zip, context: \"{}\")",
                    e
                ))
            }
        };
        let geodata = match ggv_xml_process_xml(&xml) {
            Ok(x) => x,
            Err(e) => {
                return Err(anyhow!(
                    "reading ggv_xml failed (function: process, context: \"{}\")",
                    e
                ))
            }
        };
        Ok(geodata)
    }
    fn write(&self, _geodata: &Geodata) -> Result<String> {
        todo!("ggv_xml write support");
    }
    fn name<'a>(&self) -> &'a str {
        return "ggv_xml";
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

impl GgvXmlFormat {
    pub fn new() -> Self {
        Self { debug: 0 }
    }
}
