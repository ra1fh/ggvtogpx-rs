///
///  Geodata storage for Waypoints, Routes and Tracks
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

#[derive(Debug, Default, Clone)]
pub struct Waypoint {
    latitude: f64,
    longitude: f64,
    elevation: f64,
    name: String,
}

impl Waypoint {
    pub fn new() -> Self {
        Self {
            latitude: f64::NAN,
            longitude: f64::NAN,
            elevation: f64::NAN,
            name: String::from(""),
        }
    }
    pub fn with_lat(mut self, lat: f64) -> Self {
        self.latitude = lat;
        self
    }
    pub fn with_lon(mut self, lon: f64) -> Self {
        self.longitude = lon;
        self
    }
    pub fn with_elevation(mut self, ele: f64) -> Self {
        self.elevation = ele;
        self
    }
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
    pub fn latitude(&self) -> f64 {
        self.latitude
    }
    pub fn longitude(&self) -> f64 {
        self.longitude
    }
    pub fn elevation(&self) -> f64 {
        self.elevation
    }
    pub fn name(&self) -> String {
        self.name.to_owned()
    }
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }
}

#[derive(Debug, Default)]
pub struct WaypointList {
    waypoints: Vec<Waypoint>,
    name: String,
}

impl WaypointList {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_waypoint(&mut self, wp: Waypoint) {
        self.waypoints.push(wp)
    }
    pub fn name(&self) -> String {
        self.name.to_owned()
    }
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned()
    }
    pub fn extract_first_waypoint(&self) -> &Waypoint {
        &self.waypoints[0]
    }
    pub fn waypoints(&self) -> &Vec<Waypoint> {
        &self.waypoints
    }
    pub fn len(&self) -> usize {
        self.waypoints.len()
    }
}

#[derive(Debug)]
pub struct Geodata {
    debug: u8,
    waypoints: Vec<WaypointList>,
    routes: Vec<WaypointList>,
    tracks: Vec<WaypointList>,
}

impl Geodata {
    pub fn new() -> Self {
        Self {
            debug: 0,
            waypoints: vec![WaypointList::default()],
            routes: Vec::new(),
            tracks: Vec::new(),
        }
    }
    pub fn with_debug(mut self, value: u8) -> Self {
        self.debug = value;
        self
    }
    pub fn add_waypoint(&mut self, wp: Waypoint) {
        if self.debug >= 3 {
            eprintln!("geodata: add waypoint");
        }
        if self.waypoints.len() == 0 {
            self.waypoints.push(WaypointList::default());
        }
        self.waypoints[0].add_waypoint(wp);
    }
    pub fn add_route(&mut self, route: WaypointList) {
        if self.debug >= 3 {
            eprintln!("geodata: add route");
        }
        self.routes.push(route);
    }
    pub fn add_track(&mut self, track: WaypointList) {
        if self.debug >= 3 {
            eprintln!("geodata: add track");
        }
        self.tracks.push(track);
    }
    pub fn waypoints(&self) -> &WaypointList {
        &self.waypoints[0]
    }
    pub fn waypoints_len(&self) -> usize {
        if self.waypoints.len() > 0 {
            self.waypoints[0].len()
        } else {
            0
        }
    }
    pub fn waypoints_vec(&self) -> &Vec<WaypointList> {
        &self.waypoints
    }
    pub fn routes(&self) -> &Vec<WaypointList> {
        &self.routes
    }
    pub fn tracks(&self) -> &Vec<WaypointList> {
        &self.tracks
    }
    pub fn get_bounds(&self) -> Option<(Waypoint, Waypoint)> {
        let min_lat = 0.0;
        let max_lat = 90.0;
        let min_lon = -180.0;
        let max_lon = 180.0;
        let mut min = Waypoint::new().with_lat(max_lat).with_lon(max_lon);
        let mut max = Waypoint::new().with_lat(min_lat).with_lon(min_lon);

        let container = vec![self.waypoints_vec(), self.tracks(), self.routes()];
        let points = container
            .iter()
            .map(|wplist| {
                wplist
                    .iter()
                    .map(|w| w.waypoints.len())
                    .fold(0, |acc, len| acc + len)
            })
            .fold(0, |acc, len| acc + len);
        if points == 0 {
            return None;
        }

        for list_of_wplists in container.iter() {
            for wplist in list_of_wplists.iter() {
                for waypoint in wplist.waypoints.iter() {
                    if waypoint.latitude > max.latitude {
                        max.latitude = waypoint.latitude;
                    }
                    if waypoint.latitude < min.latitude {
                        min.latitude = waypoint.latitude;
                    }
                    if waypoint.longitude > max.longitude {
                        max.longitude = waypoint.longitude;
                    }
                    if waypoint.longitude < min.longitude {
                        min.longitude = waypoint.longitude;
                    }
                }
            }
        }
        Some((min, max))
    }
}
