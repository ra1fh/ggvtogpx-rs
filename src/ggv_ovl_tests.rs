///
///  ggv_ovl test cases
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse() {
        let test = "[section 1]\nXKoord0=10.65544468 \n \n \n[section 2]\nfoo=bar\n";
        let (rem, res) = ggv_ovl_parse(test.as_bytes()).unwrap();
        println!("test    = {:?}", test);
        println!("    res = {:?}", res);
        println!("    rem = {:?}", decode_latin1(rem));
    }

    #[test]
    fn test_parse_key_value() {
        let tests = [
            ("foo=bar", "foo", "bar", ""),
            ("foo = bar", "foo", "bar", ""),
            ("foo = bar; \n  ", "foo", "bar", "\n  "),
        ];
        for (t, k, v, r) in tests {
            let (rem, (key, val)) = ggv_ovl_parse_key_value(t.as_bytes()).unwrap();
            println!(
                "test = {:?}, key = {:?}, val = {:?}, rem = {:?}",
                t,
                key,
                val,
                decode_latin1(rem)
            );
            assert_eq!(key, k);
            assert_eq!(val, v);
            assert_eq!(rem, r.as_bytes());
        }
    }

    #[test]
    fn test_parse_section() {
        let tests = [("[Foo]", "Foo", ""), ("[Foo]  ", "Foo", "  ")];
        for (t, v, r) in tests {
            let (rem, val) = ggv_ovl_parse_section(t.as_bytes()).unwrap();
            println!(
                "test = {:?}, val = {:?}, rem = {:?}",
                t,
                val,
                decode_latin1(rem)
            );
            assert_eq!(val, v);
            assert_eq!(rem, r.as_bytes());
        }
    }
}
