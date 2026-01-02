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

use anyhow::Result;
use chrono::{DateTime, Utc};
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::writer::Writer;

use crate::format::Format;
use crate::geodata::Geodata;
use crate::geodata::Waypoint;

#[derive(Debug, Default)]
pub struct GpxFormat {
    creator: String,
    testmode: bool,
    debug: u8,
}

impl Format for GpxFormat {
    fn probe(&self, _buf: &[u8]) -> bool {
        false
    }
    fn read(&self, _buf: &[u8]) -> Result<Geodata> {
        todo!("gpx read support");
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
        false
    }
    fn can_write(&self) -> bool {
        true
    }
    fn set_debug(&mut self, debug: u8) {
        self.debug = debug;
    }
}

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
