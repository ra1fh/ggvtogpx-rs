.. image:: https://github.com/ra1fh/ggvtogpx-rs/actions/workflows/build.yml/badge.svg
    :target: https://github.com/ra1fh/ggvtogpx-rs/actions/workflows/build.yml

ggvtogpx
========

``ggvtogpx`` converts Geogrid-Viewer overlay files (OVL) to GPX. This
is a port of the `C++ version <https://github.com/ra1fh/ggvtogpx>`_ to
Rust.

Geogrid-Viewer has been part of several `Top50
<https://de.wikipedia.org/wiki/Top50>`_ topographic map products that
used to be available in Germany and Austria from regional surveyor's
offices on CD-ROM/DVD.

``ggvtogpx`` supports binary overlay file format version 2.0, 3.0 and
4.0. Those files can be identified by looking at the first bytes. The
following example shows the first bytes of a version 3.0 file:

::

    hexdump -C example.ovl | head -n 2
    00000000  44 4f 4d 47 56 43 52 44  20 4f 76 6c 66 69 6c 65  |DOMGVCRD Ovlfile|
    00000010  20 56 33 2e 30 3a 00 00  00 08 00 00 00 1e 00 00  | V3.0:..........|

``ggvtogpx`` supports ASCII overlay file format which looks like
this:

::

   [Symbol 1]
   Typ=3
   Group=1
   Col=3
   ...

``ggvtogpx`` supports ZIP compressed XML overlay files (version
5.0). The archive content can be listed with unzip to detect the
version 5.0 format. Specifically it contains the file geogrid50.xml:

::

   Archive:  ggv_xml-sample-1.ovl
     Length      Date    Time    Name
   ---------  ---------- -----   ----
          16  11-21-2011 07:18   Bindata0.dat
       37982  11-21-2011 07:18   geogrid50.xml
   ---------                     -------
       37998                     2 files

(Note: ``ggvtogpx`` will unpack the OVL 5.0 files, there is no need to
manually extract the XML).


Building and Installing
-----------------------

Requirements:

* Rust
* CMake for integration tests

Compilation:

::

   cmake .
   make
   make test

Installation:

::

   make install

Usage
-----

::

    Geogrid-Viewer to GPX Converter.

    Usage: ggvtogpx [OPTIONS] [infile] [outfile]

    Arguments:
      [infile]   input file (alternative to -f)
      [outfile]  output file (alternative to -F)

    Options:
      -D <debug>     debug <level> (0..5)
      -i <type>      input <type> [possible values: ggv_bin, ggv_ovl, ggv_xml, gpx]
      -f <file>      input <file>
      -o <type>      output <type> [possible values: ggv_ovl, gpx]
      -F <file>      output <file>
      -d <path>      datafile <path>
      -h, --help     Print help
      -V, --version  Print version

The input type will be automatically detected. There is typically no
need to use the -i option to overwrite the auto-detection. The
output type default to GPX. Example:

::

    ggvtogpx input.ovl output.gpx



OVL File Format
---------------

This is a pseudo C code like structure definition of OVL version 2.0,
3.0 and 4.0 files. The file type and version can be identified by
looking at the first 23 bytes of the file, which contain the
string ``"DOMGVCRD Ovlfile"`` followed by a version number.


Version 2.0
'''''''''''

.. code:: c

    // Geogrid-Viewer OVL binary file format version 2.0

    struct FILE {
      struct HEADER {
        char magic[23];    // "DOMGVCRD Ovlfile V2.0:\0"
        uint16 header_len; // usually 0x90 or 0x00
        if (header_len > 4) {
          uint16 i1;
          uint16 i2;
          char map_name[header_len - 4];
          // the map_name field contains more information
          // then just the null-terminated name, but details
          // are unknown
        }
      }
      struct RECORD[] {
        uint16 entry_type;
        // 0x02: text
        // 0x03: line
        // 0x04: area
        // 0x05: rectangle
        // 0x06: circle
        // 0x07: triangle
        // 0x09: bitmap
        uint16 entry_group;
        uint16 entry_zoom;
        uint16 entry_subtype;
        if (subtype != 0x01) {
          uint32 text_len;
          char text[text_len];
        }
        union {
          struct TEXT {
            uint16 color;
            uint16 size;
            uint16 trans;
            uint16 font;
            uint16 angle;
            double lon;
            double last;
            uint16 text_len;
            char text[text_len];
          }
          struct LINE_AREA {
            uint16 color;
            uint16 width;
            uint16 type;
            uint16 point_count;
            struct POINT {
              double lon;
              double lat;
            }[point_count];
          }
          struct RECT_CIRC_TRI {
            uint16 color;
            uint16 prop1;
            uint16 prop2;
            uint16 angle;
            uint16 stroke;
            uint16 area;
            double lon;
            double lat;
          }
          struct BITMAP {
            uint16 color;
            uint16 prop1;
            uint16 prop2;
            uint16 prop3;
            double lon;
            double lat;
            uint32 data_len;
            char data[data_len];
          }
        }
      }
    }


Version 3.0 and 4.0
'''''''''''''''''''

Version 3.0 and 4.0 are a bit different as they allow multiple parts
in one file that all start with the magic bytes ``"DOMGVCRD Ovlfile V3.0"``
or ``"DOMGVCRD Ovlfile V4.0"``. In addition, there are two types
of data sections. A \"label\" section that might contain group
definitions or similar.  And a \"record\" section that contains the
real data, like tracks or other kind of geometric objects.

The header contains the number of \"label\" and \"record\" sections
following the normal header (see label\_count and record\_count). The
counts might be zero, which means the part does not contain any label
or record sections.

.. code:: c

    struct FILE {
      // A version 3.0/4.0 file might contain multiple parts all
      // starting with DOMGCRD magic bytes and header
      struct PART[] {
        struct HEADER {
          char magic[23];
          // either "DOMGVCRD Ovlfile V3.0:\0"
          //     or "DOMGVCRD Ovlfile V4.0:\0"
          char padding[8];
          uint32 label_count;
          uint32 record_count;
          uint16 text_len;
          uint16 text[text_len];
          uint16 i1;
          uint16 i2;
          uint16 i3;
          uint16 header_len; // usually 0x90 or 0x00
          uint16 i4;
          uint16 i5;
          if (header_len > 4) {
            uint16 i1;
            uint16 i2;
            char   map_name[header_len - 4];
            // the map_name field contains more information
            // then just the null-terminated name, but details
            // are unknown
          }
        }
        struct LABEL[label_count] {
          char label_header[8];
          char label_number[14];
          uint16 label_text_len;
          char label_text[label_text_len];
          uint16 label_flags1;
          uint16 label_flags2;
        };
        struct RECORD[record_count] {
          uint16 record_type;
          // 0x02: text
          // 0x03: line
          // 0x04: area
          // 0x05: rectangle
          // 0x06: circle
          // 0x07: triangle
          // 0x09: bitmap
          // 0x17: line
          uint16 record_prop1;
          uint16 record_prop2;
          uint16 record_prop3;
          uint16 record_prop4;
          uint16 record_prop5;
          uint16 record_prop6;
          uint16 record_prop7;
          uint16 record_prop8;
          uint16 record_flags;      // 0x0001=ZOOM, 0x0002=NOZOOM, 0x0800=ROUNDED, 0x10000=CLOSED
          uint16 record_prop10;
          uint16 record_text_len;
          char record_text[record_text_len];
          uint16 record_type1;
          if (record_type1 != 1) {
            uint32 record_object1_len;
            char record_object1[record_object1_len];
          }
          uint16 record_type2;
          if (record_type2 != 1) {
            uint32 record_object2_len;
            char record_object2[record_object2_len];
          }
          union {
            struct TEXT {
              uint16 text_prop1;
              uint32 text_prop2;
              uint16 text_prop3;
              uint32 text_color;    // 0x80bbggrr
              uint16 text_size;     // 100-1100
              uint16 text_back;     // 1=transparent, 2=solid, 3-8=various patterns
              uint16 text_font;     // 1=Arial, 3=Courier, 4=Times, 10=Comic
              uint16 text_angle;    // 100-460
              double lon;
              double lat;
              double unkown;
              uint16 text_label_len;
              char [text_label_len];
            }
            struct AREA_LINE {
              uint16 line_prop1;
              uint32 line_prop2;
              uint16 line_prop3;    // 0x1e
              uint32 line_color;    // 0x80bbggrr
              uint16 line_width;    // 101-115
              uint16 line_back;     // 1=transparent, 2=solid, 3-8=various patterns
              uint16 line_count;
              if (record_type == 0x04)
                uint16 line_stroke; // 1=solid, 2=dashed, 3=dotted, 4=dot-dash
              }
              struct COORD[line_count] {
                double lon;
                double lat;
                double unkown;
            }
            struct RECT_CIRC_TRI {
              uint16 rct_prop1;
              uint32 rct_prop2;
              uint16 rct_prop3;     // 0x1e
              uint32 rct_color;     // 0x80bbggrr
              uint32 rct_width;
              uint32 rct_height;
              uint16 rct_stroke;    // 1=solid, 2=dashed, 3=dotted, 4=dot-dash
              uint16 rct_angle;     // 0-360
              uint16 rct_lwidth;    // 101-115
              uint16 rct_back;      // 1=transparent, 2=solid, 3-8=various patterns
              double lon;
              double lat;
              double unkown;
            }
            struct BITMAP {
              uint16 bmp_prop1;
              uint32 bmp_prop2;
              uint16 bmp_prop3;     // 0x1e
              uint32 bmp_prop4;
              uint32 bmp_width;
              uint32 bmp_height;
              double lon;
              double lat;
              double unkown;
              uint32 bmp_len;
              uint16 bmp_angle;     // 100-460
              char bmp_data[bmp_len];
            }
          }
        }
      }
    }

History
-------

I developed the initial OVL binary file format converter in 2016. The
code was integrated into `GPSBabel <https://www.gpsbabel.org>`_ in
January 2016 as the ``ggv_bin`` format.

In 2022, GPSBabel retired several infrequently used formats, including
``ggv_bin``. Consequently, I migrated the code into a new project
focused exclusively on OVL to GPX conversion.  For completeness, I
added support for the OVL ASCII format (``ggv_ovl``) and developed the
XML format support from scratch.

In 2025, the ggvtogpx C++ codebase was ported to Rust.

In 2026, GPX read support as well as ASCII OVL write support was added.
