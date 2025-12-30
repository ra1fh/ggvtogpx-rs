///
///  Custom error class to pass error string up the call chain
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
use nom::error::ContextError;
use nom::error::ErrorKind;
use nom::error::ParseError;

#[derive(Debug)]
pub struct CustomError {
    message: String,
}

impl CustomError {
    pub fn new(m: &str) -> Self {
        Self {
            message: String::from(m),
        }
    }
    pub fn message(&self) -> &String {
        &self.message
    }
}

impl<I> ParseError<I> for CustomError {
    fn from_error_kind(_input: I, _kind: ErrorKind) -> Self {
        CustomError {
            message: String::new(),
        }
    }
    fn append(_input: I, kind: ErrorKind, mut other: Self) -> Self {
        other.message.push_str(&format!("; Error {:?}", kind));
        other
    }
}

impl<I> ContextError<I> for CustomError {
    fn add_context(_input: I, ctx: &'static str, other: Self) -> Self {
        if other.message.is_empty() {
            CustomError {
                message: format!("{}", ctx),
            }
        } else {
            CustomError {
                message: format!("{}, {}", other.message, ctx),
            }
        }
    }
}
