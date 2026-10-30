//! Gemeinsamer PDF-Satz der Beispiele (`idg_brief`, `rekurs_findings`): A4-Fliesstext
//! mit `lopdf`, eigenem Zeilenumbruch, Helvetica-Breitentabelle, WinAnsi-Encoding und
//! **klickbaren `/Link`-Annotationen** (URI-Actions).

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream, StringFormat};

// ---- Seitengeometrie (A4 in PDF-Punkten) --------------------------------
const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;
const LEFT: f64 = 62.0;
const RIGHT: f64 = 533.0;
const TOP: f64 = PAGE_H - 66.0; // Grundlinie der ersten Zeile
const BOT: f64 = 64.0; // unterster zulässiger Grundlinien-Wert
const F_REG: &str = "F1";
const F_BOLD: &str = "F2";
const LINK: [f64; 3] = [0.043, 0.373, 0.647]; // #0B5FA5
const BLACK: [f64; 3] = [0.0, 0.0, 0.0];

// ---- Textmodell ---------------------------------------------------------
/// Ein Textstück; `link` indexiert in die URL-Liste (`None` = kein Link).
pub struct Span {
    pub t: String,
    pub link: Option<usize>,
}
pub enum Flow {
    Para { size: f64, leading: f64, after: f64, bold: bool, indent: f64, spans: Vec<Span> },
    Gap(f64),
    Rule,
}

/// Sammelt Spans + URLs bequem ein.
pub struct Doc {
    pub urls: Vec<String>,
    flows: Vec<Flow>,
}
impl Doc {
    pub fn new() -> Self {
        Self { urls: Vec::new(), flows: Vec::new() }
    }
    pub fn url(&mut self, u: &str) -> usize {
        self.urls.push(u.to_string());
        self.urls.len() - 1
    }
    pub fn para(&mut self, size: f64, leading: f64, after: f64, bold: bool, indent: f64, spans: Vec<Span>) {
        self.flows.push(Flow::Para { size, leading, after, bold, indent, spans });
    }
    pub fn gap(&mut self, h: f64) {
        self.flows.push(Flow::Gap(h));
    }
    pub fn rule(&mut self) {
        self.flows.push(Flow::Rule);
    }
}
pub fn t(s: &str) -> Span {
    Span { t: s.to_string(), link: None }
}

// ---- WinAnsi + Breiten --------------------------------------------------
/// UTF-8 → WinAnsi-Byte (deckt Umlaute, §, deutsche Anführungszeichen ab).
fn winansi_byte(c: char) -> u8 {
    match c {
        '\u{2013}' | '\u{2014}' => 0x96,    // en/em dash → WinAnsi en dash (–)
        '\u{2022}' => 0x95,                 // • Bullet
        '\u{201E}' => 0x84,                 // „
        '\u{201C}' => 0x93,                 // "
        '\u{201D}' => 0x94,                 // "
        '\u{2018}' => 0x91,                 // '
        '\u{2019}' => 0x92,                 // '
        '\u{2026}' => 0x85,                 // …
        '\u{00A0}' => b' ',                 // NBSP
        c if (c as u32) <= 0xFF => c as u8, // Latin-1 inkl. § (0xA7), Umlaute
        _ => b'?',
    }
}
pub fn winansi(s: &str) -> Vec<u8> {
    s.chars().map(winansi_byte).collect()
}

/// Helvetica-Zeichenbreite (AFM, 1000-em-Einheiten) für ein WinAnsi-Byte.
fn glyph_w_reg(b: u8) -> f64 {
    match b {
        32 => 278.0, 33 => 278.0, 34 => 355.0, 35 => 556.0, 36 => 556.0, 37 => 889.0,
        38 => 667.0, 39 => 191.0, 40 => 333.0, 41 => 333.0, 42 => 389.0, 43 => 584.0,
        44 => 278.0, 45 => 333.0, 46 => 278.0, 47 => 278.0, 48..=57 => 556.0, 58 => 278.0,
        59 => 278.0, 60 => 584.0, 61 => 584.0, 62 => 584.0, 63 => 556.0, 64 => 1015.0,
        65 => 667.0, 66 => 667.0, 67 => 722.0, 68 => 722.0, 69 => 667.0, 70 => 611.0,
        71 => 778.0, 72 => 722.0, 73 => 278.0, 74 => 500.0, 75 => 667.0, 76 => 556.0,
        77 => 833.0, 78 => 722.0, 79 => 778.0, 80 => 667.0, 81 => 778.0, 82 => 722.0,
        83 => 667.0, 84 => 611.0, 85 => 722.0, 86 => 667.0, 87 => 944.0, 88 => 667.0,
        89 => 667.0, 90 => 611.0, 91 => 278.0, 92 => 278.0, 93 => 278.0, 94 => 469.0,
        95 => 556.0, 96 => 333.0, 97 => 556.0, 98 => 556.0, 99 => 500.0, 100 => 556.0,
        101 => 556.0, 102 => 278.0, 103 => 556.0, 104 => 556.0, 105 => 222.0, 106 => 222.0,
        107 => 500.0, 108 => 222.0, 109 => 833.0, 110 => 556.0, 111 => 556.0, 112 => 556.0,
        113 => 556.0, 114 => 333.0, 115 => 500.0, 116 => 278.0, 117 => 556.0, 118 => 500.0,
        119 => 722.0, 120 => 500.0, 121 => 500.0, 122 => 500.0, 123 => 334.0, 124 => 260.0,
        125 => 334.0, 126 => 584.0,
        0x84 | 0x91 | 0x92 | 0x93 | 0x94 => 333.0, // Anführungszeichen (‚/'/'/"/")
        0x85 => 1000.0,                             // …
        0x95 => 350.0,                              // • Bullet
        0x96 => 556.0,                              // – en dash
        0xA7 => 556.0,                              // §
        0xC4 => 667.0, 0xD6 => 778.0, 0xDC => 722.0, // Ä Ö Ü
        0xE4 => 556.0, 0xF6 => 556.0, 0xFC => 556.0, // ä ö ü
        0xDF => 556.0,                              // ß
        _ => 556.0,
    }
}
/// Dasselbe für Helvetica-Bold (breitere Glyphen — nötig, damit Segmente in der
/// fetten Betreffzeile nicht überlappen).
fn glyph_w_bold(b: u8) -> f64 {
    match b {
        32 => 278.0, 33 => 333.0, 34 => 474.0, 35 => 556.0, 36 => 556.0, 37 => 889.0,
        38 => 722.0, 39 => 238.0, 40 => 333.0, 41 => 333.0, 42 => 389.0, 43 => 584.0,
        44 => 278.0, 45 => 333.0, 46 => 278.0, 47 => 278.0, 48..=57 => 556.0, 58 => 333.0,
        59 => 333.0, 60 => 584.0, 61 => 584.0, 62 => 584.0, 63 => 611.0, 64 => 975.0,
        65 => 722.0, 66 => 722.0, 67 => 722.0, 68 => 722.0, 69 => 667.0, 70 => 611.0,
        71 => 778.0, 72 => 722.0, 73 => 278.0, 74 => 556.0, 75 => 722.0, 76 => 611.0,
        77 => 833.0, 78 => 722.0, 79 => 778.0, 80 => 667.0, 81 => 778.0, 82 => 722.0,
        83 => 667.0, 84 => 611.0, 85 => 722.0, 86 => 667.0, 87 => 944.0, 88 => 667.0,
        89 => 667.0, 90 => 611.0, 91 => 333.0, 92 => 278.0, 93 => 333.0, 94 => 584.0,
        95 => 556.0, 96 => 333.0, 97 => 556.0, 98 => 611.0, 99 => 556.0, 100 => 611.0,
        101 => 556.0, 102 => 333.0, 103 => 611.0, 104 => 611.0, 105 => 278.0, 106 => 278.0,
        107 => 556.0, 108 => 278.0, 109 => 889.0, 110 => 611.0, 111 => 611.0, 112 => 611.0,
        113 => 611.0, 114 => 389.0, 115 => 556.0, 116 => 333.0, 117 => 611.0, 118 => 556.0,
        119 => 778.0, 120 => 556.0, 121 => 556.0, 122 => 500.0, 123 => 389.0, 124 => 280.0,
        125 => 389.0, 126 => 584.0,
        0x84 | 0x91 | 0x92 | 0x93 | 0x94 => 500.0, // Anführungszeichen
        0x85 => 1000.0,                             // …
        0x95 => 350.0,                              // • Bullet
        0x96 => 556.0,                              // – en dash
        0xA7 => 556.0,                              // §
        0xC4 => 722.0, 0xD6 => 778.0, 0xDC => 722.0, // Ä Ö Ü
        0xE4 => 556.0, 0xF6 => 611.0, 0xFC => 611.0, // ä ö ü
        0xDF => 611.0,                              // ß
        _ => 611.0,
    }
}
fn char_w(c: char, size: f64, bold: bool) -> f64 {
    let g = if bold { glyph_w_bold(winansi_byte(c)) } else { glyph_w_reg(winansi_byte(c)) };
    g * size / 1000.0
}
fn width(chars: &[(char, Option<usize>)], size: f64, bold: bool) -> f64 {
    chars.iter().map(|&(c, _)| char_w(c, size, bold)).sum()
}

// ---- Zeilenumbruch ------------------------------------------------------
/// Bricht einen Absatz in Zeilen um: bevorzugt an Leerzeichen, notfalls
/// (überlange „Wörter" wie URLs) hart im Wort. Erhält die Link-Zuordnung je Zeichen.
fn wrap(chars: &[(char, Option<usize>)], size: f64, maxw: f64, bold: bool) -> Vec<Vec<(char, Option<usize>)>> {
    let mut lines: Vec<Vec<(char, Option<usize>)>> = Vec::new();
    let mut line: Vec<(char, Option<usize>)> = Vec::new();
    let mut w = 0.0;
    let mut last_space: Option<usize> = None; // Index in `line` direkt nach einem Space

    let trim = |mut l: Vec<(char, Option<usize>)>| {
        while matches!(l.last(), Some(&(' ', _))) {
            l.pop();
        }
        l
    };

    for &(c, link) in chars {
        let cw = char_w(c, size, bold);
        if w + cw > maxw && !line.is_empty() {
            if c == ' ' {
                lines.push(trim(std::mem::take(&mut line)));
                w = 0.0;
                last_space = None;
                continue;
            }
            match last_space {
                Some(bi) if bi < line.len() => {
                    let tail = line.split_off(bi);
                    lines.push(trim(std::mem::take(&mut line)));
                    line = tail;
                    w = width(&line, size, bold);
                    last_space = None;
                }
                _ => {
                    lines.push(std::mem::take(&mut line));
                    w = 0.0;
                    last_space = None;
                }
            }
        }
        line.push((c, link));
        w += cw;
        if c == ' ' {
            last_space = Some(line.len());
        }
    }
    if !line.is_empty() {
        lines.push(trim(line));
    }
    lines
}

// ---- Seiten-Renderer ----------------------------------------------------
pub struct Page {
    ops: Vec<Operation>,
    annots: Vec<(f64, f64, f64, f64, usize)>, // rect + URL-Index
}
impl Page {
    pub fn new() -> Self {
        Self { ops: Vec::new(), annots: Vec::new() }
    }
}

fn set_color(ops: &mut Vec<Operation>, c: [f64; 3]) {
    ops.push(Operation::new("rg", vec![c[0].into(), c[1].into(), c[2].into()]));
}

/// Zeichnet eine Zeile ab Grundlinie `y`, gruppiert nach Link, sammelt Link-Rechtecke.
fn draw_line(page: &mut Page, line: &[(char, Option<usize>)], x0: f64, y: f64, size: f64, bold: bool) {
    let font = if bold { F_BOLD } else { F_REG };
    let mut x = x0;
    let mut i = 0;
    while i < line.len() {
        let link = line[i].1;
        let mut j = i;
        while j < line.len() && line[j].1 == link {
            j += 1;
        }
        let seg: String = line[i..j].iter().map(|&(c, _)| c).collect();
        let seg_w = width(&line[i..j], size, bold);
        let color = if link.is_some() { LINK } else { BLACK };
        set_color(&mut page.ops, color);
        page.ops.push(Operation::new("BT", vec![]));
        page.ops.push(Operation::new("Tf", vec![font.into(), size.into()]));
        page.ops.push(Operation::new("Td", vec![x.into(), y.into()]));
        page.ops.push(Operation::new("Tj", vec![Object::String(winansi(&seg), StringFormat::Literal)]));
        page.ops.push(Operation::new("ET", vec![]));
        if let Some(idx) = link {
            // Unterstreichung
            let uy = y - 0.14 * size;
            let uh = (size * 0.05).max(0.5);
            set_color(&mut page.ops, LINK);
            page.ops.push(Operation::new(
                "re",
                vec![x.into(), uy.into(), seg_w.into(), uh.into()],
            ));
            page.ops.push(Operation::new("f", vec![]));
            // klickbares Rechteck
            page.annots.push((x, y - 0.22 * size, x + seg_w, y + 0.80 * size, idx));
        }
        x += seg_w;
        i = j;
    }
}

pub fn build(doc: &Doc) -> Vec<Page> {
    let maxw = RIGHT - LEFT;
    let mut pages = vec![Page::new()];
    let mut y = TOP;
    macro_rules! cur {
        () => {
            pages.last_mut().unwrap()
        };
    }
    for flow in &doc.flows {
        match flow {
            Flow::Gap(h) => {
                y -= h;
                if y < BOT {
                    pages.push(Page::new());
                    y = TOP;
                }
            }
            Flow::Rule => {
                if y < BOT + 12.0 {
                    pages.push(Page::new());
                    y = TOP;
                }
                let c = 0.6;
                set_color(&mut cur!().ops, [c, c, c]);
                cur!().ops.push(Operation::new("re", vec![LEFT.into(), y.into(), (RIGHT - LEFT).into(), 0.6.into()]));
                cur!().ops.push(Operation::new("f", vec![]));
                y -= 10.0;
            }
            Flow::Para { size, leading, after, bold, indent, spans } => {
                // Spans → Zeichenstrom mit Link-Index
                let mut chars: Vec<(char, Option<usize>)> = Vec::new();
                for s in spans {
                    for c in s.t.chars() {
                        chars.push((c, s.link));
                    }
                }
                let x0 = LEFT + indent;
                let lines = wrap(&chars, *size, maxw - indent, *bold);
                for line in &lines {
                    if y - *leading < BOT {
                        pages.push(Page::new());
                        y = TOP;
                    }
                    draw_line(cur!(), line, x0, y, *size, *bold);
                    y -= *leading;
                }
                y -= *after;
            }
        }
    }
    pages
}

/// Verlinkter Span (URL-Index aus [`Doc::url`]).
pub fn lk(s: &str, i: usize) -> Span {
    Span { t: s.to_string(), link: Some(i) }
}

// ---- PDF-Zusammenbau ----------------------------------------------------
/// Setzt `doc` und schreibt das PDF nach `out`; `page_numbers` ergänzt eine
/// Fusszeile «Seite n / N». Gibt die Seitenzahl zurück.
pub fn save(doc: &Doc, out: &str, title: &str, author: &str, page_numbers: bool) -> usize {
    let mut pages = build(doc);
    if page_numbers {
        let n = pages.len();
        for (i, page) in pages.iter_mut().enumerate() {
            let label = format!("Seite {} / {}", i + 1, n);
            let chars: Vec<(char, Option<usize>)> = label.chars().map(|c| (c, None)).collect();
            let x = RIGHT - width(&chars, 8.0, false);
            draw_line(page, &chars, x, BOT - 30.0, 8.0, false);
        }
    }

    let mut pdf = Document::with_version("1.5");
    let pages_id = pdf.new_object_id();
    let f_reg = pdf.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let f_bold = pdf.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = pdf.add_object(dictionary! {
        "Font" => dictionary! { F_REG => f_reg, F_BOLD => f_bold },
    });

    let mut kids: Vec<Object> = Vec::new();
    for page in &pages {
        let content = Content { operations: page.ops.clone() };
        let stream_id = pdf.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

        let mut annot_refs: Vec<Object> = Vec::new();
        for &(x0, y0, x1, y1, idx) in &page.annots {
            let a = pdf.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Link",
                "Rect" => vec![x0.into(), y0.into(), x1.into(), y1.into()],
                "Border" => vec![0.into(), 0.into(), 0.into()],
                "H" => "N",
                "A" => dictionary! {
                    "S" => "URI",
                    "URI" => Object::String(doc.urls[idx].clone().into_bytes(), StringFormat::Literal),
                },
            });
            annot_refs.push(a.into());
        }

        let mut page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_W.into(), PAGE_H.into()],
            "Resources" => resources_id,
            "Contents" => stream_id,
        };
        if !annot_refs.is_empty() {
            page_dict.set("Annots", annot_refs);
        }
        let page_id = pdf.add_object(page_dict);
        kids.push(page_id.into());
    }

    let count = kids.len() as i64;
    pdf.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );
    let catalog_id = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    pdf.trailer.set("Root", catalog_id);
    pdf.trailer.set(
        "Info",
        dictionary! {
            "Title" => Object::String(winansi(title), StringFormat::Literal),
            "Author" => Object::String(winansi(author), StringFormat::Literal),
        },
    );

    pdf.save(out).expect("PDF speichern");
    pages.len()
}
