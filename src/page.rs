//! Paper setup: page size, orientation and margins.
//!
//! A page size is either an ISO 216 / 269 format (`A4`, `B5`, `C5`, ...),
//! a North American format (`Letter`, `Legal`, `Tabloid`), a custom size
//! given as [`Measure`]s, or [`PageSize::Fill`]: no paper, the editor fills
//! the parent container.
//!
//! ```
//! use iced_markdown_editor::{Margins, Measure, Paper, PageSize};
//!
//! let a4 = Paper::default();                                  // A4 portrait, 2.5 cm margins
//! let a5 = Paper::new(PageSize::A5).landscape();
//! let custom = Paper::new(PageSize::custom(Measure::mm(180.0), Measure::mm(240.0)))
//!     .with_margins(Margins::all(Measure::cm(1.5)));
//! let full = Paper::fill();
//! let parsed: PageSize = "A4".parse().unwrap();
//! let parsed: PageSize = "21cm x 29.7cm".parse().unwrap();
//! ```

use std::fmt;
use std::str::FromStr;

/// Logical pixels per inch (the CSS / Windows reference resolution).
pub const DPI: f32 = 96.0;

/// Unit of a [`Measure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Millimeter,
    Centimeter,
    Inch,
    /// Typographic point (1/72 inch).
    Point,
    /// Logical pixel (1/96 inch).
    Pixel,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Millimeter => "mm",
            Unit::Centimeter => "cm",
            Unit::Inch => "in",
            Unit::Point => "pt",
            Unit::Pixel => "px",
        }
    }
}

/// A physical length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measure {
    pub value: f32,
    pub unit: Unit,
}

impl Measure {
    pub const fn new(value: f32, unit: Unit) -> Self {
        Self { value, unit }
    }

    pub const fn mm(value: f32) -> Self {
        Self::new(value, Unit::Millimeter)
    }

    pub const fn cm(value: f32) -> Self {
        Self::new(value, Unit::Centimeter)
    }

    pub const fn inches(value: f32) -> Self {
        Self::new(value, Unit::Inch)
    }

    pub const fn pt(value: f32) -> Self {
        Self::new(value, Unit::Point)
    }

    pub const fn px(value: f32) -> Self {
        Self::new(value, Unit::Pixel)
    }

    /// The length in logical pixels at 100 % zoom.
    pub fn to_px(self) -> f32 {
        match self.unit {
            Unit::Millimeter => self.value / 25.4 * DPI,
            Unit::Centimeter => self.value / 2.54 * DPI,
            Unit::Inch => self.value * DPI,
            Unit::Point => self.value / 72.0 * DPI,
            Unit::Pixel => self.value,
        }
    }

    /// The length in millimetres.
    pub fn to_mm(self) -> f32 {
        self.to_px() / DPI * 25.4
    }
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.unit.symbol())
    }
}

/// Error returned when parsing a [`Measure`] or [`PageSize`] fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

fn split_number(text: &str) -> (&str, &str) {
    let end = text
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == ','))
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    (&text[..end], text[end..].trim())
}

fn parse_unit(text: &str) -> Option<Unit> {
    match text.to_ascii_lowercase().as_str() {
        "mm" => Some(Unit::Millimeter),
        "cm" => Some(Unit::Centimeter),
        "in" | "inch" | "inches" | "\"" => Some(Unit::Inch),
        "pt" => Some(Unit::Point),
        "px" => Some(Unit::Pixel),
        _ => None,
    }
}

impl FromStr for Measure {
    type Err = ParseError;

    /// Parses `"25mm"`, `"2.5 cm"`, `"1in"`, `"72pt"`, `"96px"`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let (number, unit) = split_number(text);
        let value: f32 = number
            .replace(',', ".")
            .parse()
            .map_err(|_| ParseError(format!("invalid number in `{text}`")))?;
        let unit = parse_unit(unit)
            .ok_or_else(|| ParseError(format!("missing or unknown unit in `{text}`")))?;
        Ok(Self::new(value, unit))
    }
}

/// ISO 216 / 269 series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IsoSeries {
    A,
    B,
    C,
}

const ISO_A: [(f32, f32); 11] = [
    (841.0, 1189.0),
    (594.0, 841.0),
    (420.0, 594.0),
    (297.0, 420.0),
    (210.0, 297.0),
    (148.0, 210.0),
    (105.0, 148.0),
    (74.0, 105.0),
    (52.0, 74.0),
    (37.0, 52.0),
    (26.0, 37.0),
];

const ISO_B: [(f32, f32); 11] = [
    (1000.0, 1414.0),
    (707.0, 1000.0),
    (500.0, 707.0),
    (353.0, 500.0),
    (250.0, 353.0),
    (176.0, 250.0),
    (125.0, 176.0),
    (88.0, 125.0),
    (62.0, 88.0),
    (44.0, 62.0),
    (31.0, 44.0),
];

const ISO_C: [(f32, f32); 11] = [
    (917.0, 1297.0),
    (648.0, 917.0),
    (458.0, 648.0),
    (324.0, 458.0),
    (229.0, 324.0),
    (162.0, 229.0),
    (114.0, 162.0),
    (81.0, 114.0),
    (57.0, 81.0),
    (40.0, 57.0),
    (28.0, 40.0),
];

/// Page size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
    /// ISO format, e.g. `Iso(IsoSeries::A, 4)` for A4. Numbers 0-10.
    Iso(IsoSeries, u8),
    /// US Letter, 8.5 × 11 in.
    Letter,
    /// US Legal, 8.5 × 14 in.
    Legal,
    /// Tabloid, 11 × 17 in.
    Tabloid,
    /// Any size (use [`Orientation`] to decide which side is longer).
    Custom { width: Measure, height: Measure },
    /// No paper: the editor fills 100 % of the parent container's width and
    /// height. Paper margins are ignored; no sheet, no page breaks.
    Fill,
}

impl PageSize {
    pub const A3: PageSize = PageSize::Iso(IsoSeries::A, 3);
    pub const A4: PageSize = PageSize::Iso(IsoSeries::A, 4);
    pub const A5: PageSize = PageSize::Iso(IsoSeries::A, 5);
    pub const A6: PageSize = PageSize::Iso(IsoSeries::A, 6);
    pub const B4: PageSize = PageSize::Iso(IsoSeries::B, 4);
    pub const B5: PageSize = PageSize::Iso(IsoSeries::B, 5);
    pub const C5: PageSize = PageSize::Iso(IsoSeries::C, 5);

    /// ISO format; the number is clamped to 0-10.
    pub fn iso(series: IsoSeries, number: u8) -> Self {
        PageSize::Iso(series, number.min(10))
    }

    pub fn custom(width: Measure, height: Measure) -> Self {
        PageSize::Custom { width, height }
    }

    /// Dimensions as given, or `None` for [`PageSize::Fill`].
    pub fn dimensions(&self) -> Option<(Measure, Measure)> {
        match *self {
            PageSize::Iso(series, number) => {
                let table = match series {
                    IsoSeries::A => &ISO_A,
                    IsoSeries::B => &ISO_B,
                    IsoSeries::C => &ISO_C,
                };
                let (w, h) = table[number.min(10) as usize];
                Some((Measure::mm(w), Measure::mm(h)))
            }
            PageSize::Letter => Some((Measure::inches(8.5), Measure::inches(11.0))),
            PageSize::Legal => Some((Measure::inches(8.5), Measure::inches(14.0))),
            PageSize::Tabloid => Some((Measure::inches(11.0), Measure::inches(17.0))),
            PageSize::Custom { width, height } => Some((width, height)),
            PageSize::Fill => None,
        }
    }
}

impl fmt::Display for PageSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageSize::Iso(series, number) => write!(f, "{series:?}{number}"),
            PageSize::Letter => f.write_str("Letter"),
            PageSize::Legal => f.write_str("Legal"),
            PageSize::Tabloid => f.write_str("Tabloid"),
            PageSize::Custom { width, height } => write!(f, "{width} x {height}"),
            PageSize::Fill => f.write_str("Fill"),
        }
    }
}

impl FromStr for PageSize {
    type Err = ParseError;

    /// Parses `"A4"`, `"b5"`, `"Letter"`, `"legal"`, `"tabloid"`,
    /// `"fill"` / `"full"`, or a custom size like `"210mm x 297mm"`,
    /// `"21 x 29.7 cm"`, `"8.5in x 11in"`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let lower = text.trim().to_ascii_lowercase();

        match lower.as_str() {
            "letter" => return Ok(PageSize::Letter),
            "legal" => return Ok(PageSize::Legal),
            "tabloid" | "ledger" => return Ok(PageSize::Tabloid),
            "fill" | "full" | "fullscreen" | "full screen" | "full width" => {
                return Ok(PageSize::Fill);
            }
            _ => {}
        }

        let mut chars = lower.chars();
        if let Some(first) = chars.next() {
            let series = match first {
                'a' => Some(IsoSeries::A),
                'b' => Some(IsoSeries::B),
                'c' => Some(IsoSeries::C),
                _ => None,
            };
            if let Some(series) = series {
                if let Ok(number) = chars.as_str().parse::<u8>() {
                    if number <= 10 {
                        return Ok(PageSize::Iso(series, number));
                    }
                }
            }
        }

        let parts: Vec<&str> = lower.split(['x', '×', '*']).map(str::trim).collect();
        if parts.len() == 2 {
            let height: Measure = parts[1].parse()?;
            let width = match parts[0].parse::<Measure>() {
                Ok(width) => width,
                Err(_) => {
                    let value: f32 = parts[0]
                        .replace(',', ".")
                        .parse()
                        .map_err(|_| ParseError(format!("invalid width in `{text}`")))?;
                    Measure::new(value, height.unit)
                }
            };
            return Ok(PageSize::Custom { width, height });
        }

        Err(ParseError(format!("unknown page size `{text}`")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Page margins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: Measure,
    pub right: Measure,
    pub bottom: Measure,
    pub left: Measure,
}

impl Margins {
    pub fn new(top: Measure, right: Measure, bottom: Measure, left: Measure) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn all(margin: Measure) -> Self {
        Self::new(margin, margin, margin, margin)
    }

    pub fn symmetric(vertical: Measure, horizontal: Measure) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}

impl Default for Margins {
    /// 2.5 cm on every side.
    fn default() -> Self {
        Self::all(Measure::cm(2.5))
    }
}

/// Complete page setup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    pub size: PageSize,
    pub orientation: Orientation,
    pub margins: Margins,
}

impl Default for Paper {
    /// A4 portrait with 2.5 cm margins.
    fn default() -> Self {
        Self::new(PageSize::A4)
    }
}

impl Paper {
    pub fn new(size: PageSize) -> Self {
        Self {
            size,
            orientation: Orientation::Portrait,
            margins: Margins::default(),
        }
    }

    pub fn a4() -> Self {
        Self::new(PageSize::A4)
    }

    pub fn letter() -> Self {
        Self::new(PageSize::Letter).with_margins(Margins::all(Measure::inches(1.0)))
    }

    /// No paper: 100 % width and height of the parent container.
    pub fn fill() -> Self {
        Self::new(PageSize::Fill)
    }

    pub fn with_size(mut self, size: PageSize) -> Self {
        self.size = size;
        self
    }

    pub fn with_orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn landscape(self) -> Self {
        self.with_orientation(Orientation::Landscape)
    }

    pub fn portrait(self) -> Self {
        self.with_orientation(Orientation::Portrait)
    }

    pub fn with_margins(mut self, margins: Margins) -> Self {
        self.margins = margins;
        self
    }

    pub fn is_fill(&self) -> bool {
        self.size == PageSize::Fill
    }

    /// Page size in logical pixels at 100 % zoom, orientation applied.
    /// `None` for [`PageSize::Fill`].
    pub fn size_px(&self) -> Option<(f32, f32)> {
        let (w, h) = self.size.dimensions()?;
        let (w, h) = (w.to_px(), h.to_px());
        let (short, long) = (w.min(h), w.max(h));
        Some(match self.orientation {
            Orientation::Portrait => (short, long),
            Orientation::Landscape => (long, short),
        })
    }
}
