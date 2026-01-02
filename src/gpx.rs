///
/// Support for GPX writing
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
use std::error::Error;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::writer::Writer;

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

#[derive(Debug, Default)]
pub struct GpxFormat {
    creator: String,
    testmode: bool,
    debug: u8,
}

fn gpx_read_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    let Some(n) = node.children().find(|c| c.has_tag_name(tag)) else {
        return None;
    };
    let Some(t) = n.text() else {
        return None;
    };
    Some(String::from(t))
}

fn gpx_read_waypoint(node: roxmltree::Node) -> Option<Waypoint> {
    let lat = node.attribute("lat")?;
    let lat = lat.parse::<f64>().ok()?;
    let lon = node.attribute("lon")?;
    let lon = lon.parse::<f64>().ok()?;
    let ele = gpx_read_text(node, "ele").and_then(|v| v.parse::<f64>().ok());
    match ele {
        Some(ele) => Some(
            Waypoint::new()
                .with_lat(lat)
                .with_lon(lon)
                .with_elevation(ele),
        ),
        _ => Some(Waypoint::new().with_lat(lat).with_lon(lon)),
    }
}

fn gpx_read_trk(trk: roxmltree::Node, geodata: &mut Geodata) {
    let name = gpx_read_text(trk, "name").unwrap_or(String::from(""));
    let mut list = WaypointList::new();
    list.set_name(&name);
    for trkseg in trk.children().filter(|c| c.has_tag_name("trkseg")) {
        for trkpt in trkseg.children().filter(|c| c.has_tag_name("trkpt")) {
            let Some(waypoint) = gpx_read_waypoint(trkpt) else {
                continue;
            };
            list.add_waypoint(waypoint);
        }
    }
    geodata.add_track(list);
}

fn gpx_read_rte(rte: roxmltree::Node, geodata: &mut Geodata) {
    let name = gpx_read_text(rte, "name").unwrap_or(String::from(""));
    let mut list = WaypointList::new();
    list.set_name(&name);
    for rtept in rte.children().filter(|c| c.has_tag_name("rtept")) {
        let Some(mut waypoint) = gpx_read_waypoint(rtept) else {
            continue;
        };
        let name = gpx_read_text(rtept, "name").unwrap_or(String::from(""));
        if !name.is_empty() {
            waypoint.set_name(&name);
        }
        list.add_waypoint(waypoint);
    }
    geodata.add_route(list);
}

fn gpx_read_wpt(wpt: roxmltree::Node, geodata: &mut Geodata) {
    let Some(mut waypoint) = gpx_read_waypoint(wpt) else {
        return;
    };
    let name = gpx_read_text(wpt, "name").unwrap_or(String::from(""));
    if !name.is_empty() {
        waypoint.set_name(&name);
        geodata.add_waypoint(waypoint);
        return;
    }
    let cmt = gpx_read_text(wpt, "cmt").unwrap_or(String::from(""));
    if !cmt.is_empty() {
        waypoint.set_name(&cmt);
        geodata.add_waypoint(waypoint);
        return;
    }
    let desc = gpx_read_text(wpt, "desc").unwrap_or(String::from(""));
    if !desc.is_empty() {
        waypoint.set_name(&desc);
        geodata.add_waypoint(waypoint);
        return;
    }
    geodata.add_waypoint(waypoint);
}

/// Parse gpx xml
fn gpx_process_xml<'a>(xml: &str) -> Result<Geodata> {
    let mut geodata = Geodata::new().with_debug(get_debug());
    let doc = roxmltree::Document::parse(xml).with_context(|| "parse xml")?;
    let root = doc.root().first_child().with_context(|| "root node")?;
    root.has_tag_name("gpx")
        .then_some(())
        .ok_or_else(|| anyhow!("gpx tag"))?;
    for trk in root.children().filter(|c| c.has_tag_name("trk")) {
        gpx_read_trk(trk, &mut geodata);
    }
    for rte in root.children().filter(|c| c.has_tag_name("rte")) {
        gpx_read_rte(rte, &mut geodata);
    }
    for wpt in root.children().filter(|c| c.has_tag_name("wpt")) {
        gpx_read_wpt(wpt, &mut geodata);
    }
    Ok(geodata)
}

//////////////////////////////////////////////////////////////////////
//            entry points called by ggvtogpx main process
//////////////////////////////////////////////////////////////////////

impl Format for GpxFormat {
    fn probe(&self, buf: &[u8]) -> bool {
	let Ok(s) = std::str::from_utf8(buf) else {
	    return false;
	};
	let Ok(doc) = roxmltree::Document::parse(s).with_context(|| "parse xml") else {
	    return false;
	};
	let Ok(root) = doc.root().first_child().with_context(|| "root node") else {
	    return false;
	};
	if ! root.has_tag_name("gpx") {
	    return false;
	}
	return true
    }
    fn read(&self, buf: &[u8]) -> Result<Geodata> {
        let str = std::str::from_utf8(buf)?;
        gpx_process_xml(str)
    }
    fn write(&self, geodata: &Geodata) -> Result<String> {
        let mut buffer = Vec::new();
        let mut writer = Writer::new_with_indent(&mut buffer, b' ', 2);
        let epoch = DateTime::from_timestamp_secs(0).expect("invalid timestmap");
        let now = Utc::now();

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .expect("writing decl");
        writer
            .create_element("gpx")
            .with_attribute(("version", "1.0"))
            .with_attribute(("creator", self.creator.as_str()))
            .with_attribute(("xmlns", "http://www.topografix.com/GPX/1/0"))
            .write_inner_content(|writer| {
                if self.testmode {
                    writer
                        .create_element("time")
                        .write_text_content(BytesText::new(
                            format!("{}", epoch.format("%Y-%m-%dT%H:%M:%S%:z")).as_str(),
                        ))?;
                } else {
                    writer
                        .create_element("time")
                        .write_text_content(BytesText::new(
                            format!("{}", now.format("%Y-%m-%dT%H:%M:%S%:z")).as_str(),
                        ))?;
                }
                match geodata.get_bounds() {
                    Some(bounds) => {
                        let (min, max) = bounds;
                        writer
                            .create_element("bounds")
                            .with_attribute(("minlat", (format!("{:.9}", min.latitude())).as_str()))
                            .with_attribute((
                                "minlon",
                                (format!("{:.9}", min.longitude())).as_str(),
                            ))
                            .with_attribute(("maxlat", (format!("{:.9}", max.latitude())).as_str()))
                            .with_attribute((
                                "maxlon",
                                (format!("{:.9}", max.longitude())).as_str(),
                            ))
                            .write_empty()?;
                    }
                    _ => (),
                }

                for waypoint in geodata.waypoints().waypoints().iter() {
                    Self::write_waypoint(writer, &waypoint, "wpt", true).expect("write wpt failed");
                }
                for route in geodata.routes().iter() {
                    writer.create_element("rte").write_inner_content(|writer| {
                        if !route.name().is_empty() {
                            writer
                                .create_element("name")
                                .write_text_content(BytesText::new(route.name().as_str()))?;
                        }
                        for waypoint in route.waypoints().iter() {
                            Self::write_waypoint(writer, &waypoint, "rtept", false)
                                .expect("write rtept failed");
                        }
                        Ok(())
                    })?;
                }
                for track in geodata.tracks().iter() {
                    writer.create_element("trk").write_inner_content(|writer| {
                        if !track.name().is_empty() {
                            writer
                                .create_element("name")
                                .write_text_content(BytesText::new(track.name().as_str()))?;
                        }
                        writer
                            .create_element("trkseg")
                            .write_inner_content(|writer| {
                                for waypoint in track.waypoints().iter() {
                                    Self::write_waypoint(writer, &waypoint, "trkpt", false)
                                        .expect("write trkpt failed");
                                }
                                Ok(())
                            })?;
                        Ok(())
                    })?;
                }
                Ok(())
            })?;
        let output = std::str::from_utf8(&buffer)?;
        Ok(output.to_string() + "\n")
    }
    fn name<'a>(&self) -> &'a str {
        return "gpx";
    }
    fn can_read(&self) -> bool {
        true
    }
    fn can_write(&self) -> bool {
        true
    }
    fn set_debug(&mut self, debug: u8) {
        self.debug = debug;
        set_debug(debug);
    }
}

//////////////////////////////////////////////////////////////////////
//            Additional GPX specific member functions
//////////////////////////////////////////////////////////////////////

impl GpxFormat {
    pub fn new() -> Self {
        Self::default()
            .with_creator(&env::var("GGVTOGPX_CREATOR").unwrap_or("ggvtogpx".to_string()))
            .with_testmode(if env::var("GGVTOGPX_TESTMODE").is_ok() {
                true
            } else {
                false
            })
    }
    pub fn with_creator(mut self, creator: &str) -> Self {
        self.creator = creator.to_owned();
        self
    }
    pub fn with_testmode(mut self, testmode: bool) -> Self {
        self.testmode = testmode;
        self
    }
    pub fn write_waypoint(
        writer: &mut Writer<&mut Vec<u8>>,
        waypoint: &Waypoint,
        element: &str,
        cmt_desc: bool,
    ) -> Result<(), Box<dyn Error>> {
        if waypoint.name().is_empty() && waypoint.elevation().is_nan() {
            writer
                .create_element(element)
                .with_attribute(("lat", format!("{:.9}", waypoint.latitude()).as_str()))
                .with_attribute(("lon", format!("{:.9}", waypoint.longitude()).as_str()))
                .write_empty()?;
            Ok(())
        } else {
            writer
                .create_element(element)
                .with_attribute(("lat", format!("{:.9}", waypoint.latitude()).as_str()))
                .with_attribute(("lon", format!("{:.9}", waypoint.longitude()).as_str()))
                .write_inner_content(|writer| {
                    if !waypoint.elevation().is_nan() {
                        writer
                            .create_element("ele")
                            .write_text_content(BytesText::new(&format!(
                                "{:.9}",
                                waypoint.elevation()
                            )))?;
                    }
                    if !waypoint.name().is_empty() {
                        writer
                            .create_element("name")
                            .write_text_content(BytesText::new(&waypoint.name()))?;
                        if cmt_desc {
                            writer
                                .create_element("cmt")
                                .write_text_content(BytesText::new(&waypoint.name()))?;
                            writer
                                .create_element("desc")
                                .write_text_content(BytesText::new(&waypoint.name()))?;
                        }
                    }
                    Ok(())
                })?;
            Ok(())
        }
    }
}
