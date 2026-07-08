//! Erzeugt das **IDG-Zugangsgesuch an das Kantonale Steueramt Zürich** als PDF —
//! reiner Rust-Code mit `lopdf` (wie `src/pdf_report.rs`), aber zusätzlich mit
//! **klickbaren `/Link`-Annotationen** (URI-Actions) auf die Gesetzesgrundlagen,
//! eigenem Zeilenumbruch, Helvetica-Breitentabelle und WinAnsi-Encoding.
//!
//!   cargo run --example idg_brief                 # -> ~/idg-zugangsgesuch.pdf
//!   cargo run --example idg_brief -- /pfad/x.pdf  # eigener Ausgabepfad

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

// ---- verifizierte Fundstellen (Gesetzesgrundlagen / Quellen) ------------
const U_KV: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-101-2005_02_27-2006_01_01-103.html";
const U_IDG: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-170_4-2007_02_12-2008_10_01-109.html";
const U_STG: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-631_1-1997_06_08-1999_01_01-111.html";
const U_VO: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-631_121-2011_10_18-2013_01_01-129.html";
const U_ZSTB: &str = "https://www.zh.ch/de/steuern-finanzen/steuern/treuhaender/steuerbuch/steuerbuch-definition/zstb-109c-4.html";
const U_OEFF: &str = "https://www.zh.ch/de/politik-staat/kanton/kantonale-verwaltung/oeffentlichkeitsprinzip.html";
const U_FAQ: &str = "https://www.zh.ch/content/dam/zhweb/bilder-dokumente/themen/steuern-finanzen/steuern/juristischepersonen/fragen-antworten-zhcorporatetax-barrierefrei.pdf";
const U_DSB: &str = "https://www.datenschutz.ch/";

// ---- Textmodell ---------------------------------------------------------
/// Ein Textstück; `link` indexiert in die URL-Liste (`None` = kein Link).
struct Span {
    t: String,
    link: Option<usize>,
}
enum Flow {
    Para { size: f64, leading: f64, after: f64, bold: bool, indent: f64, spans: Vec<Span> },
    Gap(f64),
    Rule,
}

/// Sammelt Spans + URLs bequem ein.
struct Doc {
    urls: Vec<String>,
    flows: Vec<Flow>,
}
impl Doc {
    fn new() -> Self {
        Self { urls: Vec::new(), flows: Vec::new() }
    }
    fn url(&mut self, u: &str) -> usize {
        self.urls.push(u.to_string());
        self.urls.len() - 1
    }
    fn para(&mut self, size: f64, leading: f64, after: f64, bold: bool, indent: f64, spans: Vec<Span>) {
        self.flows.push(Flow::Para { size, leading, after, bold, indent, spans });
    }
    fn gap(&mut self, h: f64) {
        self.flows.push(Flow::Gap(h));
    }
    fn rule(&mut self) {
        self.flows.push(Flow::Rule);
    }
}
fn t(s: &str) -> Span {
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
fn winansi(s: &str) -> Vec<u8> {
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
struct Page {
    ops: Vec<Operation>,
    annots: Vec<(f64, f64, f64, f64, usize)>, // rect + URL-Index
}
impl Page {
    fn new() -> Self {
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

fn build(doc: &Doc) -> Vec<Page> {
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

// ---- Inhalt des Briefes -------------------------------------------------
fn letter() -> Doc {
    let mut d = Doc::new();
    let (kv, idg, stg, vo, oeff, dsb) =
        (d.url(U_KV), d.url(U_IDG), d.url(U_STG), d.url(U_VO), d.url(U_OEFF), d.url(U_DSB));
    let lk = |s: &str, i: usize| Span { t: s.to_string(), link: Some(i) };

    // Absender / Empfänger (linksbündig, klein)
    for l in ["[Firma / Absender]", "[Vorname Name]", "[Strasse Nr.]", "[PLZ Ort]", "[E-Mail]"] {
        d.para(10.0, 13.0, 1.0, false, 0.0, vec![t(l)]);
    }
    d.gap(12.0);
    for l in ["Kantonales Steueramt Zürich", "[Ansprechperson]", "[Adresse]", "[PLZ Ort]"] {
        d.para(10.0, 13.0, 1.0, false, 0.0, vec![t(l)]);
    }
    d.gap(12.0);
    d.para(10.0, 13.0, 1.0, false, 0.0, vec![t("[Ort], [Datum]")]);
    d.gap(12.0);

    // Betreff (fett, mit Links)
    d.para(11.0, 15.0, 12.0, true, 0.0, vec![
        t("Formelles Gesuch um Zugang zu amtlichen Dokumenten ("),
        lk("Art. 17 KV ZH", kv),
        t("; "),
        lk("§§ 20 ff. IDG", idg),
        t(") — API-Beschrieb der Steuerbehörde für natürliche und juristische Personen (NP und JP)"),
    ]);

    d.para(10.5, 15.0, 6.0, false, 0.0, vec![t("Sehr geehrte Damen und Herren")]);

    let body = 10.5;
    let lead = 15.0;
    let after = 9.0;
    d.para(body, lead, after, false, 0.0, vec![t(
        "Vorab halte ich fest: Die Freigabe des Onboardings und mein Anspruch auf Zugang zu \
         amtlichen Dokumenten sind zwei rechtlich voneinander unabhängige Fragen. Das Erste \
         liegt in Ihrem Ermessen — das Zweite nicht.",
    )]);

    d.para(body, lead, after, false, 0.0, vec![
        t("Der API-Beschrieb der Steuerbehörde für natürliche und juristische Personen (NP und JP) \
           ist ein amtliches Dokument im Sinne des IDG. Gestützt auf "),
        lk("Art. 17 der Verfassung des Kantons Zürich", kv),
        t(" sowie "),
        lk("§§ 20 ff. des Gesetzes über die Information und den Datenschutz (IDG)", idg),
        t(" ersuche ich Sie hiermit förmlich um Zugang zu diesen Dokumenten."),
    ]);

    d.para(body, lead, after, false, 0.0, vec![
        t("Nach dem "),
        lk("Öffentlichkeitsprinzip", oeff),
        t(" hat jede Person Anspruch auf Zugang zu amtlichen Dokumenten — ohne Nachweis eines \
           Interesses und ohne Angabe von Gründen ("),
        lk("§ 20 IDG", idg),
        t("). Die von Ihnen verlangten Angaben (Business-Plan, Timeline, B2B/B2C-Konzept, \
           Firmenstruktur) sind onboarding-interne Kriterien und bilden keine gesetzliche \
           Voraussetzung für den Zugang zu amtlichen Dokumenten. Der Zugang darf davon nicht \
           abhängig gemacht werden."),
    ]);

    d.para(body, lead, after, false, 0.0, vec![
        t("Zur Rechtslage: Die elektronische Einreichung der Steuererklärung stützt sich auf "),
        lk("§§ 109c, 109d und 133 des Steuergesetzes", stg),
        t(" in Verbindung mit der "),
        lk("Verordnung über die elektronische Einreichung der Steuererklärung (LS 631.121)", vo),
        t(". Diese Verordnung kennt weder ein Zulassungsverfahren für Drittsoftware noch die von \
           Ihnen verlangten Angaben. Für gewerbsmässige Steuervertreter hält § 12 Abs. 2 der \
           Verordnung sogar ausdrücklich fest, dass — über die Prüfung der \
           Unternehmens-Identifikationsnummer hinaus — „keine weiteren Abklärungen“ vorgenommen werden."),
    ]);

    d.para(body, lead, after, false, 0.0, vec![t(
        "Ich ersuche Sie daher, mir die konkrete Rechtsgrundlage (Erlass, Paragraph/Artikel) zu \
         nennen, auf die Sie sich stützen, wenn Sie den Zugang zur Schnittstellendokumentation von \
         der Vorlage eines Business-Plans, einer Timeline, eines B2B/B2C-Konzepts sowie der \
         Firmenstruktur und dem Entscheid eines Gremiums abhängig machen. Fehlt eine solche \
         Grundlage, ist die Koppelung an mein Zugangsgesuch nach IDG nicht zulässig.",
    )]);

    d.para(body, lead, after, false, 0.0, vec![t(
        "Die betreffenden Angaben stelle ich Ihnen kulanzhalber und ohne Anerkennung einer \
         Rechtspflicht zur Verfügung, damit das Onboarding parallel vorangetrieben werden kann; \
         sie präjudizieren meinen Zugangsanspruch nicht und schränken ihn nicht ein.",
    )]);

    d.para(body, lead, after, false, 0.0, vec![
        t("Ich ersuche Sie, dieses Schreiben als förmliches Zugangsgesuch nach IDG entgegenzunehmen \
           und mir innert der gesetzlichen Frist Zugang zu gewähren. Eine ganze oder teilweise \
           Verweigerung erwarte ich als schriftlichen, begründeten Entscheid unter Nennung des \
           konkreten Ausnahmetatbestands ("),
        lk("§ 23 IDG", idg),
        t("). Für diesen Fall behalte ich mir ausdrücklich die Anrufung der "),
        lk("Datenschutzbeauftragten des Kantons Zürich", dsb),
        t(" (Schlichtung/Empfehlung) sowie den Rekurs vor."),
    ]);

    d.gap(6.0);
    d.para(body, lead, 2.0, false, 0.0, vec![t("Freundliche Grüsse")]);
    d.gap(20.0);
    d.para(10.0, 13.0, 1.0, false, 0.0, vec![t("[Vorname Name]")]);
    d.para(10.0, 13.0, 1.0, false, 0.0, vec![t("[Firma / Absender]")]);

    d.gap(14.0);
    d.rule();
    d.gap(4.0);
    d.para(10.5, 14.0, 6.0, true, 0.0, vec![t("Rechtsgrundlagen und Quellen")]);

    let quellen = [
        ("Verfassung des Kantons Zürich (LS 101 / SR 131.211), Art. 17 — Öffentlichkeit staatlicher Tätigkeit", U_KV),
        ("Gesetz über die Information und den Datenschutz (IDG, LS 170.4), § 20 Zugangsrecht, § 23 Einschränkungen", U_IDG),
        ("Steuergesetz (StG, LS 631.1), §§ 109c / 109d / 133", U_STG),
        ("Verordnung über die elektronische Einreichung der Steuererklärung (LS 631.121), § 12 Abs. 2 (Treuhänder-Register)", U_VO),
        ("Zürcher Steuerbuch — ZStB 109c.4 (Kommentar zur Verordnung 631.121)", U_ZSTB),
        ("Öffentlichkeitsprinzip — Kanton Zürich (Erläuterung)", U_OEFF),
        ("ZHcorporateTax — Fragen & Antworten (Anbindung von Drittsoftware als „separates Projekt“)", U_FAQ),
        ("Datenschutzbeauftragte des Kantons Zürich (Schlichtung / Empfehlung)", U_DSB),
    ];
    for (label, u) in quellen {
        let i = d.url(u);
        d.para(9.0, 12.0, 1.0, false, 0.0, vec![t("•  "), t(label)]);
        d.para(8.5, 11.5, 6.0, false, 14.0, vec![Span { t: u.to_string(), link: Some(i) }]);
    }

    d
}

// ---- PDF-Zusammenbau ----------------------------------------------------
fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/idg-zugangsgesuch.pdf")
    });

    let letter = letter();
    let pages = build(&letter);

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let f_reg = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let f_bold = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { F_REG => f_reg, F_BOLD => f_bold },
    });

    let mut kids: Vec<Object> = Vec::new();
    for page in &pages {
        let content = Content { operations: page.ops.clone() };
        let stream_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

        let mut annot_refs: Vec<Object> = Vec::new();
        for &(x0, y0, x1, y1, idx) in &page.annots {
            let a = doc.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Link",
                "Rect" => vec![x0.into(), y0.into(), x1.into(), y1.into()],
                "Border" => vec![0.into(), 0.into(), 0.into()],
                "H" => "N",
                "A" => dictionary! {
                    "S" => "URI",
                    "URI" => Object::String(letter.urls[idx].clone().into_bytes(), StringFormat::Literal),
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
        let page_id = doc.add_object(page_dict);
        kids.push(page_id.into());
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc.trailer.set(
        "Info",
        dictionary! {
            "Title" => Object::String(winansi("Gesuch um Zugang zu amtlichen Dokumenten (IDG)"), StringFormat::Literal),
            "Author" => Object::String(winansi("[Absender]"), StringFormat::Literal),
        },
    );

    doc.save(&out).expect("PDF speichern");
    println!("OK -> {out}  ({} Seite[n])", pages.len());
}
