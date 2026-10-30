//! Erzeugt das **IDG-Zugangsgesuch an das Kantonale Steueramt Zürich** als PDF —
//! reiner Rust-Code mit `lopdf` (wie `src/pdf_report.rs`), aber zusätzlich mit
//! **klickbaren `/Link`-Annotationen** (URI-Actions) auf die Gesetzesgrundlagen,
//! eigenem Zeilenumbruch, Helvetica-Breitentabelle und WinAnsi-Encoding.
//!
//!   cargo run --example idg_brief                 # -> ~/idg-zugangsgesuch.pdf
//!   cargo run --example idg_brief -- /pfad/x.pdf  # eigener Ausgabepfad

#[path = "common/linkpdf.rs"]
#[allow(dead_code)]
mod linkpdf;
use linkpdf::{lk, save, t, Doc, Span};

// ---- verifizierte Fundstellen (Gesetzesgrundlagen / Quellen) ------------
const U_KV: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-101-2005_02_27-2006_01_01-103.html";
const U_IDG: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-170_4-2007_02_12-2008_10_01-109.html";
const U_STG: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-631_1-1997_06_08-1999_01_01-111.html";
const U_VO: &str = "https://www.zh.ch/de/politik-staat/gesetze-beschluesse/gesetzessammlung/zhlex-ls/erlass-631_121-2011_10_18-2013_01_01-129.html";
const U_ZSTB: &str = "https://www.zh.ch/de/steuern-finanzen/steuern/treuhaender/steuerbuch/steuerbuch-definition/zstb-109c-4.html";
const U_OEFF: &str = "https://www.zh.ch/de/politik-staat/kanton/kantonale-verwaltung/oeffentlichkeitsprinzip.html";
const U_FAQ: &str = "https://www.zh.ch/content/dam/zhweb/bilder-dokumente/themen/steuern-finanzen/steuern/juristischepersonen/fragen-antworten-zhcorporatetax-barrierefrei.pdf";
const U_DSB: &str = "https://www.datenschutz.ch/";

// ---- Inhalt des Briefes -------------------------------------------------
fn letter() -> Doc {
    let mut d = Doc::new();
    let (kv, idg, stg, vo, oeff, dsb) =
        (d.url(U_KV), d.url(U_IDG), d.url(U_STG), d.url(U_VO), d.url(U_OEFF), d.url(U_DSB));

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

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/idg-zugangsgesuch.pdf")
    });
    let n = save(&letter(), &out, "Gesuch um Zugang zu amtlichen Dokumenten (IDG)", "[Absender]", false);
    println!("OK -> {out}  ({n} Seite[n])");
}
