//! Recherche-Notiz zum **Rekurs D10/2026** (Zugang zum API-Beschrieb des Kantonalen
//! Steueramts Zürich): Rechtsprechung zum Öffentlichkeitsprinzip bei IT-Dokumenten und
//! der Vergleich mit swissdamed/EUDAMED, deren Schnittstellen öffentlich dokumentiert
//! sind. Alle Urteile und Quellen sind im Text verlinkt. Nur öffentliche Quellen, keine
//! Personendaten.
//!
//!   cargo run --example rekurs_findings                 # -> docs/rekurs/Recherche_…_2026-10-30.pdf
//!   cargo run --example rekurs_findings -- /pfad/x.pdf  # eigener Ausgabepfad

#[path = "common/linkpdf.rs"]
#[allow(dead_code)]
mod linkpdf;
use linkpdf::{lk, save, t, Doc, Span};

// ---- Fundstellen --------------------------------------------------------
// Schweiz
const U_BGER_ZUG: &str = "https://www.bger.ch/ext/eurospider/live/de/php/aza/http/index.php?highlight_docid=aza%3A%2F%2F02-02-2026-1C_136-2025&lang=de&type=show_document&zoom=YES";
const U_ZUG_ZW: &str = "https://www.zugerpresse.ch/zuger-news/detail/transparenz-statt-pauschalgeheimnis";
const U_ZUG_BOTE: &str = "https://www.bote.ch/nachrichten/zentralschweiz/bundesgericht-zu-geheimhaltung-von-it-dokumenten-art-1691625";
const U_ZUG_ZZ: &str = "https://www.zugerzeitung.ch/zentralschweiz/zug/bundesgericht-zu-geheimhaltung-von-it-dokumenten-ld.4128834";
const U_ZUG_ZP: &str = "https://www.zentralplus.ch/justiz/knatsch-um-gerichts-it-bundesgericht-ruegt-zuger-behoerde-2847864/";
const U_BGE142: &str = "http://relevancy.bger.ch/php/clir/http/index.php?highlight_docid=atf%3A%2F%2F142-II-313%3Ade&lang=de&type=show_document";
const U_BVGER4595: &str = "https://www.swissrights.ch/bvge/2021-A-4595/2020.php";
const U_BVGER1432: &str = "https://entscheide.weblaw.ch/cache.php?link=05-04-2017-A-1432-2016";
const U_BVGER7405: &str = "https://entscheide.weblaw.ch/cache.php?link=23-11-2015-A-7405-2014";
const U_VB2019_603: &str = "https://entscheidsuche.ch/docs/ZH_Verwaltungsgericht/ZH_VG_001_-VB-2019-00603_2019-12-19.html";
const U_VB2021_858: &str = "https://entscheidsuche.ch/dok/ZH_Verwaltungsgericht/ZH_VG_001_-VB-2021-00858_2022-06-30.html";
const U_VB2024_43: &str = "https://entscheide.weblaw.ch/dumppdf.php?link=ZH-VG-2024-12-31_VB.2024.00043";
const U_VB2022_142: &str = "https://entscheidsuche.ch/docs/ZH_Verwaltungsgericht/ZH_VG_001_-VB-2022-00142_2023-03-30.html";
const U_STEUERGEH: &str = "https://cdbf.ch/de/1323/";
const U_LU: &str = "https://www.lu.ch/verwaltung/JSD/JSD_Projekte_und_Themen/Oeffentlichkeitsprinzip";
const U_BK_OSS: &str = "https://www.bk.admin.ch/dam/de/sd-web/OmKVvt8e0eVp/Em002-6%20FAQ-OSS.pdf";
const U_BACS_OSS: &str = "https://www.ncsc.admin.ch/dam/ncsc/de/dokumente/ueber-ncsc/Open-Source-Strategie-BACS_DE.pdf.download.pdf/Open-Source-Strategie-BACS_DE.pdf";
// Medizinprodukte
const U_SM_TECH: &str = "https://www.swissmedic.ch/swissmedic/de/home/medizinprodukte/medizinprodukte-datenbank/technische-dokumentation.html";
const U_SM_API: &str = "https://www.swissmedic.ch/dam/swissmedic/de/dokumente/medizinprodukte/mep_urr/bw630_40_810e_pu_swissdamed_machine_to_machine_rest_api-documentation.pdf.download.pdf/BW630_40_810e_PU_swissdamed_Machine_to_Machine_REST_%20API_Documentation.pdf";
const U_SM_UDI: &str = "https://www.swissmedic.ch/swissmedic/de/home/medizinprodukte/medizinprodukte-datenbank/udi-eindeutige-produktidentifikation.html";
const U_WW: &str = "https://www.walderwyss.com/en/news/2026-02-20_swissmedic-publication-of-technical-documentation-for-m2m-registration-in-swissdamed";
const U_EUD_TECH: &str = "https://webgate.ec.europa.eu/eudamed-help/en/documentation/technical-documentation.html";
const U_EUD_M2M: &str = "https://webgate.ec.europa.eu/eudamed-help/en/files/M2M%20-%20user%20guide.pdf";
// Ausland
const U_TA_PARIS: &str = "https://lexing.law/wp-content/uploads/2014/05/33796487.pdf";
const U_TA_PRESSE: &str = "https://next.ink/14734/99038-la-justice-confirme-qu-un-code-source-logiciel-est-document-administratif-communicable-au-citoyen/";
const U_CADA: &str = "https://larevue.squirepattonboggs.com/cada-avis-favorable-pour-l-acces-au-code-source-d-un-logiciel-de-bercy_a2572.html";
const U_CDS_2270: &str = "https://www.agendadigitale.eu/cultura-digitale/laccesso-allalgoritmo-secondo-il-giudice-amministrativo/";
const U_CDS_PROD: &str = "https://www.agendadigitale.eu/documenti/algoritmi-nella-pa-accesso-al-software-e-diritti-del-produttore-i-paletti-del-consiglio-di-stato/";
const U_FG_BREMEN: &str = "https://www.finanzgericht-bremen.de/sixcms/media.php/13/2k_12_14_(2)_an.pdf";
const U_BT_ELSTER: &str = "https://kleineanfragen.de/bundestag/19/1058-elektronische-erledigung-von-steueranmeldungen-und-erklaerungen-mit-elster.pdf";
const U_BVERFG: &str = "https://www.bundesverfassungsgericht.de/SharedDocs/Entscheidungen/DE/2009/03/cs20090303_2bvc000307.html";
const U_VFGH: &str = "https://www.vfgh.gv.at/downloads/VfGH_V_85-96-11_e-voting.pdf";

const BODY: f64 = 10.0;
const LEAD: f64 = 14.0;

fn h1(d: &mut Doc, s: &str) {
    d.gap(6.0);
    d.para(12.5, 17.0, 6.0, true, 0.0, vec![t(s)]);
}
fn h2(d: &mut Doc, s: &str) {
    d.gap(2.0);
    d.para(10.5, 15.0, 4.0, true, 0.0, vec![t(s)]);
}
fn p(d: &mut Doc, spans: Vec<Span>) {
    d.para(BODY, LEAD, 7.0, false, 0.0, spans);
}
fn bullet(d: &mut Doc, mut spans: Vec<Span>) {
    spans.insert(0, t("•  "));
    d.para(BODY, LEAD, 5.0, false, 10.0, spans);
}

fn findings() -> Doc {
    let mut d = Doc::new();
    macro_rules! u {
        ($c:expr) => {
            d.url($c)
        };
    }

    // ---- Titel ----------------------------------------------------------
    d.para(15.0, 20.0, 4.0, true, 0.0, vec![t("Zugang zum API-Beschrieb für Steuereinreichungen")]);
    d.para(11.0, 15.0, 4.0, false, 0.0, vec![t(
        "Rechtsprechung zum Öffentlichkeitsprinzip bei IT-Dokumenten und Vergleich mit swissdamed / EUDAMED",
    )]);
    d.para(9.5, 13.0, 8.0, false, 0.0, vec![t(
        "Recherche vom 30. Oktober 2026 zum Rekurs D10/2026 (Finanzdirektion des Kantons Zürich) – \
         Arbeitsgrundlage für die Replik",
    )]);
    d.rule();
    d.para(9.0, 12.5, 8.0, false, 0.0, vec![t(
        "Hinweis zur Belastbarkeit: Die Fundstellen wurden über Websuche, Entscheid-Auszüge und \
         Sekundärquellen (Medienberichte, Fachartikel) erschlossen. Vor dem Zitieren in der Replik \
         ist jeder Entscheid im Volltext zu prüfen. Wo nur eine Sekundärquelle vorliegt, ist das \
         ausdrücklich vermerkt.",
    )]);

    // ---- 1. Ergebnis ----------------------------------------------------
    h1(&mut d, "1. Ergebnis in Kürze");
    p(&mut d, vec![t(
        "Ein Urteil, das ausdrücklich festhält, der API- oder Schnittstellenbeschrieb einer \
         Steuerverwaltung sei öffentlich zugänglich zu machen, wurde weder in der Schweiz noch im \
         Ausland gefunden. Der Rekurs D10/2026 betrifft insofern eine offene Rechtsfrage. Tragfähig sind:",
    )]);
    let bz = u!(U_BGER_ZUG);
    bullet(&mut d, vec![
        t("das Urteil des Bundesgerichts "),
        lk("1C_136/2025 vom 2. Februar 2026", bz),
        t(" (Kanton Zug), das die pauschale Verweigerung des Zugangs zu IT-Dokumenten mit einer \
           allgemeinen Sicherheitsbegründung aufhebt;"),
    ]);
    bullet(&mut d, vec![t(
        "die gefestigte Rechtsprechung zur Zugangsvermutung, zur Beweislast der Behörde und zur \
         konkreten Begründungspflicht bei Geschäftsgeheimnissen;",
    )]);
    let smt = u!(U_SM_TECH);
    let eut = u!(U_EUD_TECH);
    bullet(&mut d, vec![
        t("der Vergleich mit "),
        lk("swissdamed (Swissmedic)", smt),
        t(" und "),
        lk("EUDAMED (EU-Kommission)", eut),
        t(": Dort werden die Maschinenschnittstellen für eine gesetzliche Registrierungspflicht \
           vollständig und ohne Login veröffentlicht;"),
    ]);
    let tap = u!(U_TA_PARIS);
    let cds = u!(U_CDS_2270);
    bullet(&mut d, vec![
        t("ausländische Entscheide zum Zugang zu Verwaltungssoftware, namentlich "),
        lk("TA Paris vom 10. März 2016", tap),
        t(" (Quellcode des Steuerrechners) und "),
        lk("Consiglio di Stato Nr. 2270/2019", cds),
        t("."),
    ]);

    // ---- 2. Zug ---------------------------------------------------------
    h1(&mut d, "2. BGer 1C_136/2025 vom 2. Februar 2026 (Kanton Zug, IT-Dokumente)");
    let (zw, bote, zz, zp) = (u!(U_ZUG_ZW), u!(U_ZUG_BOTE), u!(U_ZUG_ZZ), u!(U_ZUG_ZP));
    p(&mut d, vec![
        t("Volltext: "),
        lk("Urteil 1C_136/2025 vom 2. Februar 2026, I. öffentlich-rechtliche Abteilung", bz),
        t(" (Haag, Kneubühler, Müller). Vorinstanz: Beschluss der Präsidentin des Verwaltungsgerichts \
           des Kantons Zug vom 31. Januar 2025 (K 2024 9). Medienberichte: "),
        lk("Zuger Woche", zw),
        t(", "),
        lk("Bote der Urschweiz", bote),
        t(", "),
        lk("Zuger Zeitung", zz),
        t(", "),
        lk("zentralplus", zp),
        t("."),
    ]);
    bullet(&mut d, vec![t(
        "Sachverhalt: Die Partei PARAT verlangte am 22. September 2024 vom Verwaltungsgericht Zug \
         Zugang zu den amtlichen Dokumenten über die eingesetzten Softwareprodukte und Dienstleister, \
         die Standorte der Server und Cloud-Infrastruktur, die Administration der Geräte, die \
         vertraglichen Zusicherungen, die Evaluation bei der Beschaffung und die Richtlinien zur \
         Datenbearbeitung (Zuger Öffentlichkeitsgesetz, BGS 158.1).",
    )]);
    bullet(&mut d, vec![t(
        "Vorinstanz: beantwortete einige Fragen knapp (kantonale Rechenzentren, Betrieb durch das \
         Amt für Informatik und Organisation mit ISO-27001-zertifiziertem ISMS) und verweigerte \
         jede Einsicht in Dokumente – Offenlegung könne die Datensicherheit kompromittieren, \
         Hackerangriffe und Spear-Phishing begünstigen; selbst ein eingeschränkter Zugang sei \
         nicht möglich (E. 4.1).",
    )]);
    bullet(&mut d, vec![t(
        "E. 6.3: «Aus der Begründung geht jedoch in keiner Weise hervor, auf welche Dokumente sich \
         die Vorinstanz genau bezieht, ob es sich um physische oder elektronische Dokumente handelt, \
         ob diese sich überhaupt im Besitz der Vorinstanz befinden und ob bzw. inwiefern alle Teile \
         aller betroffenen Dokumente schutzwürdig sind, inkl. die Benennung dieser Dokumente.»",
    )]);
    bullet(&mut d, vec![t(
        "E. 6.3: «Aus dem Umstand, dass die Vorinstanz einige Fragen der Beschwerdeführerin \
         beantwortet hat, ist abzuleiten, dass zumindest einige der betroffenen Dokumente \
         Informationen enthalten, die nicht schutzwürdig sind.» Die Behörde wäre verpflichtet \
         gewesen, diese Teile offenzulegen oder genauer darzulegen, weshalb auch sie nicht \
         zugänglich sind (§ 9 Abs. 2 Öffentlichkeitsgesetz ZG: Einschränkung nur des schutzwürdigen Teils).",
    )]);
    bullet(&mut d, vec![t(
        "E. 6.3: Die «sehr allgemein gehaltene Begründung» erlaube nicht zu prüfen, «ob tatsächlich \
         überwiegende öffentliche Interessen an der Geheimhaltung bestehen, oder ob die Vorinstanz \
         diese nur vorschiebt». E. 6.4: Verletzung der Begründungspflicht (Art. 112 Abs. 1 BGG, \
         Art. 29 Abs. 2 BV).",
    )]);
    bullet(&mut d, vec![t(
        "Dispositiv: Beschwerde gutgeheissen, soweit darauf einzutreten ist; Beschluss aufgehoben, \
         Rückweisung zu neuem Entscheid; keine Kosten.",
    )]);
    h2(&mut d, "Tragweite – genau lesen");
    bullet(&mut d, vec![t(
        "Das Bundesgericht hat nicht entschieden, dass IT-Dokumente herauszugeben sind. Es hat den \
         Entscheid wegen ungenügender Begründung aufgehoben (Art. 112 Abs. 3 BGG) und die Frage, ob \
         die Zugangsverweigerung willkürlich ist, offen gelassen. Die in den Medien zitierte Wendung \
         «rechtlich problematisch» steht nicht im Urteil.",
    )]);
    bullet(&mut d, vec![t(
        "Auf die Rüge nach Art. 16 Abs. 3 BV (Informationsfreiheit) trat das Gericht mangels \
         Substantiierung nicht ein (E. 5); es hält an 1C_462/2018 fest, wonach Art. 16 Abs. 3 BV \
         keine über das Öffentlichkeitsgesetz hinausgehenden Zugangsrechte gibt. Für die Replik: \
         Art. 16 BV nur mit Auseinandersetzung mit dieser Rechtsprechung anrufen, sonst auf IDG stützen.",
    )]);
    bullet(&mut d, vec![t(
        "Bezug zum Rekurs: Die Verfügung vom 23. Oktober 2026 ist am Massstab von E. 6.3 zu messen – \
         welche Dokumente genau, welche Teile schutzwürdig, weshalb kein teilweiser Zugang. Und wie im \
         Zuger Fall die beantworteten Fragen zeigt beim Steueramt die Abgabe an \
         Treuhand-Softwarelieferanten, dass die Dokumentation zumindest teilweise nicht schutzwürdig ist.",
    )]);

    // ---- 3. Weitere Schweizer Entscheide --------------------------------
    h1(&mut d, "3. Weitere Schweizer Entscheide");
    let b142 = u!(U_BGE142);
    bullet(&mut d, vec![
        lk("BGE 142 II 313 (1C_296/2015)", b142),
        t(" – ESTV-Fall: Auch gegenüber der Steuerverwaltung besteht ein subjektiver Zugangsanspruch; \
           Ausnahmen sind im Einzelfall zu prüfen. Nutzen: Steuerbehörden sind nicht ausgenommen."),
    ]);
    let b4595 = u!(U_BVGER4595);
    bullet(&mut d, vec![
        lk("BVGer A-4595/2020", b4595),
        t(" – widerlegbare Vermutung zugunsten des Zugangs. Nutzen: Die Beweislast für einen \
           Ausnahmegrund trägt das Steueramt."),
    ]);
    let b1432 = u!(U_BVGER1432);
    bullet(&mut d, vec![
        t("BGer 1C_509/2016 vom 9. Februar 2017, zitiert in "),
        lk("BVGer A-1432/2016", b1432),
        t(" – die Behörde muss «konkret und im Detail» darlegen, inwiefern ein Geschäftsgeheimnis \
           vorliegt. Nutzen: gegen die pauschale Einstufung «vertraulich»."),
    ]);
    let v603 = u!(U_VB2019_603);
    bullet(&mut d, vec![
        lk("VGr ZH VB.2019.00603", v603),
        t(" – ein pauschaler Hinweis auf ein Geheimnis genügt nicht; die Partei muss konkret \
           begründen. Nutzen: dasselbe Argument, vom Zürcher Verwaltungsgericht."),
    ]);
    let v858 = u!(U_VB2021_858);
    bullet(&mut d, vec![
        lk("VGr ZH VB.2021.00858", v858),
        t(" – § 3 Abs. 2 IDG: Aufgezeichnete Informationen zur Erfüllung einer öffentlichen Aufgabe \
           sind «Information», unabhängig von Form und Datenträger. Nutzen: Der API-Beschrieb ist \
           eine Information im Sinne des IDG."),
    ]);
    let v43 = u!(U_VB2024_43);
    bullet(&mut d, vec![
        lk("VGr ZH VB.2024.00043", v43),
        t(" – der Zugang nach § 20 IDG setzt kein besonderes Interesse voraus. Nutzen: Ein \
           Business-Plan darf keine Voraussetzung sein."),
    ]);
    let b7405 = u!(U_BVGER7405);
    bullet(&mut d, vec![
        lk("BVGer A-7405/2014", b7405),
        t(" – unterscheidet die Datenbank als technisches System von den darin enthaltenen Daten. \
           Nutzen: Verlangt ist die Beschreibung der Schnittstelle, nicht der Inhalt einer Datenbank."),
    ]);
    let bk = u!(U_BK_OSS);
    bullet(&mut d, vec![
        lk("Bundeskanzlei, FAQ Open-Source-Software", bk),
        t(" – Quellcode eines Bundesamts fällt unter das BGÖ; Herausgabe, sofern keine Ausnahme nach \
           Art. 7 BGÖ greift. (Verwaltungsauffassung, kein Gerichtsentscheid.)"),
    ]);

    // ---- 4. Gegenargumente ----------------------------------------------
    h1(&mut d, "4. Zu erwartende Gegenargumente");
    let v142 = u!(U_VB2022_142);
    bullet(&mut d, vec![
        lk("VGr ZH VB.2022.00142", v142),
        t(" (Kantonspolizei): Zugang zu «eingesetzten Mitteln», auch technischen, nach § 23 Abs. 2 \
           IDG verweigert. Abgrenzung: Dort ging es um Ermittlungsmittel; hier um eine \
           Einreichungsschnittstelle, die gerade für Bürgerinnen, Bürger und Software bestimmt ist."),
    ]);
    let sg = u!(U_STEUERGEH);
    bullet(&mut d, vec![
        t("Steuergeheimnis ("),
        lk("BGer 1C_272/2022, Besprechung", sg),
        t("): geht dem Öffentlichkeitsprinzip vor, schützt aber Daten von Steuerpflichtigen – \
           nicht eine technische Spezifikation, die keine Steuerdaten enthält."),
    ]);
    let lu = u!(U_LU);
    bullet(&mut d, vec![
        lk("Kanton Luzern, Merkblatt Öffentlichkeitsprinzip", lu),
        t(": kein Anspruch auf Datenbankinhalte, keine Herausgabe von IT-Risikoanalysen. Trifft den \
           Antrag nicht: Verlangt sind weder Datenbankinhalte noch Risikoanalysen."),
    ]);

    // ---- 5. swissdamed / EUDAMED ----------------------------------------
    h1(&mut d, "5. Vergleich: swissdamed und EUDAMED");
    let udi = u!(U_SM_UDI);
    p(&mut d, vec![
        t("Die Lage ist dieselbe wie beim Steueramt: Private müssen gesetzlich verpflichtet regulierte \
           Daten maschinell bei einer Behörde einreichen – bei Medizinprodukten die "),
        lk("UDI-Registrierung nach Art. 17 MepV", udi),
        t(", verpflichtend seit 1. Juli 2026."),
    ]);
    h2(&mut d, "swissdamed (Swissmedic)");
    let api = u!(U_SM_API);
    bullet(&mut d, vec![
        t("Auf der Seite «"),
        lk("Technische Dokumentation", smt),
        t("» ohne Login: "),
        lk("BW630_40_810e Machine-to-Machine REST API Documentation", api),
        t(" (02.09.2026), BW630_40_002e Business Rules (02.09.2026), BW630_40_001e UDI Data \
           Dictionary (XLSX, 19.06.2026) sowie Verweis auf das EUDAMED-XSD v3.0.30 für den XML-Upload."),
    ]);
    let ww = u!(U_WW);
    bullet(&mut d, vec![
        t("OpenAPI-Spezifikation direkt auf dem Server (/v3/api-docs/udi-m2m-v1) und öffentliche \
           Testumgebung «Playground» seit März 2026 (laut API-Dokument und "),
        lk("Walder Wyss, 20.02.2026", ww),
        t(")."),
    ]);
    h2(&mut d, "EUDAMED (EU-Kommission)");
    bullet(&mut d, vec![
        t("Auf der Seite «"),
        lk("Technical documentation", eut),
        t("» ohne Login (Plattform v2.27.0, 24.07.2026): XSD schemas.zip (v3.0.30), XML-Beispiele für \
           Behörden, Wirtschaftsakteure und Benannte Stellen, eine Swagger/OpenAPI-Datei (.yaml), \
           Data Dictionaries, Business Rules und Service-Definitionen."),
    ]);
    let m2m = u!(U_EUD_M2M);
    bullet(&mut d, vec![
        t("Für den Produktivanschluss gibt es ein Formular «Technical onboarding» und ein \
           «Third-party agreement» – als offene Vorlagen. Architektur und Ablauf beschreibt der "),
        lk("M2M User Guide", m2m),
        t("."),
    ]);
    h2(&mut d, "Folgerungen für die Replik");
    bullet(&mut d, vec![t(
        "Dokumentation und Produktionszugang sind getrennt: EUDAMED verlangt ein technisches \
         Onboarding für den Anschluss, die Dokumentation ist aber frei. Dieselbe Trennung kann vom \
         Steueramt verlangt werden. Einen Business-Plan verlangt keine der beiden Stellen.",
    )]);
    bullet(&mut d, vec![t(
        "Das Sicherheitsargument trägt nicht: Eine Bundesbehörde und die EU-Kommission veröffentlichen \
         für Gesundheitsdaten vollständige M2M-Spezifikationen. Die Sicherheit liegt bei \
         Authentifizierung, Zertifikaten und Rate-Limits, nicht in der Geheimhaltung der Dokumentation. \
         Vgl. BGer 1C_136/2025 E. 6.3: Eine allgemein gehaltene Sicherheitsbegründung genügt nicht.",
    )]);
    bullet(&mut d, vec![t(
        "Im Steuerbereich selbst sind die Schemas eCH-0119 (Steuererklärung natürliche Personen) und \
         eCH-0217 (MWST-Abrechnung, Import bei der ESTV) öffentlich. Zurückgehalten wird nur die \
         Zürcher Übermittlungsschnittstelle.",
    )]);
    bullet(&mut d, vec![t(
        "Gleichbehandlung: Swissmedic und EUDAMED stellen die Dokumentation allen gleich zur \
         Verfügung. Das Steueramt gibt sie nach Business-Case an ausgewählte \
         Treuhand-Softwarelieferanten ab – das widerspricht der Einstufung als vertraulich \
         (Rekurs, Abschnitt 3).",
    )]);

    // ---- 6. Ausland -----------------------------------------------------
    h1(&mut d, "6. Ausländische Entscheide (Rechtsvergleich)");
    h2(&mut d, "Frankreich");
    let (tapr, cada) = (u!(U_TA_PRESSE), u!(U_CADA));
    bullet(&mut d, vec![
        lk("Tribunal administratif de Paris, 10. März 2016, Nr. 1508951/5-2", tap),
        t(", nach "),
        lk("CADA-Gutachten Nr. 20144578 vom 8. Januar 2015", cada),
        t(": Der Quellcode des Einkommenssteuer-Rechners der Steuerverwaltung DGFiP ist ein \
           herausgabepflichtiges Verwaltungsdokument; der Einwand «unfertiges Dokument» wurde \
           verworfen, jede Version des Codes ist ein Verwaltungsdokument ("),
        lk("Bericht Next INpact", tapr),
        t("). Danach veröffentlichte die DGFiP den Code. Der Schluss auf den Schnittstellenbeschrieb \
           ist ein Argument a maiore ad minus, kein direkter Präzedenzfall."),
    ]);
    h2(&mut d, "Italien");
    let prod = u!(U_CDS_PROD);
    bullet(&mut d, vec![
        lk("Consiglio di Stato, Sez. VI, Nr. 2270/2019 vom 8. April 2019", cds),
        t(", weitergeführt in Nr. 2936/2019 und 8474/2019 ("),
        lk("Besprechung", prod),
        t("): Zugang zum Quellcode eines Verwaltungsalgorithmus nach Art. 22 L. 241/1990; die \
           technische Regel sei eine allgemeine Verwaltungsregel und müsse von aussen erkennbar sein. \
           Geschäftsgeheimnisse des Herstellers werden abgewogen, schliessen den Zugang aber nicht aus."),
    ]);
    h2(&mut d, "Deutschland und Österreich");
    let (fgb, bt) = (u!(U_FG_BREMEN), u!(U_BT_ELSTER));
    bullet(&mut d, vec![
        t("ELSTER: Die Schnittstelle (ERiC) können Softwarehersteller frei nutzen; Schnittstellen und \
           Dokumentation stehen in einem «geschützten Bereich» ("),
        lk("FG Bremen 2 K 12/14", fgb),
        t("; "),
        lk("Bundestag, Kleine Anfrage 19/1058", bt),
        t("). Volltext des FG-Urteils und genaue Rechtsgrundlage (§ 87b AO / StDÜV) nicht geprüft – \
           eher Praxisvergleich als Präjudiz."),
    ]);
    let (bvg, vfgh) = (u!(U_BVERFG), u!(U_VFGH));
    bullet(&mut d, vec![
        lk("BVerfG 2 BvC 3/07 (Wahlcomputer, 2009)", bvg),
        t(" und "),
        lk("VfGH V 85/11 (E-Voting, 2011)", vfgh),
        t(": verlangen öffentliche Überprüfbarkeit staatlicher IT-Verfahren – allerdings im \
           Wahlrecht."),
    ]);

    // ---- 7. Rechtsgrundlagen --------------------------------------------
    h1(&mut d, "7. Nicht abschliessend geprüfte Rechtsgrundlagen");
    let bacs = u!(U_BACS_OSS);
    bullet(&mut d, vec![
        t("EMBAG Art. 9 (Offenlegung von Quellcode, seit 1. Januar 2024 verbindlich, vgl. "),
        lk("Open-Source-Strategie BACS", bacs),
        t("), Art. 10 (Open Government Data) und Art. 13 (Schnittstellen) gelten für die \
           Bundesverwaltung, nicht direkt für das kantonale Steueramt. Als Wertung anführbar; \
           Wortlaut von Art. 13 nicht geprüft."),
    ]);
    bullet(&mut d, vec![t(
        "Rechtsgrundlage der EUDAMED-Veröffentlichung in der MDR (vermutlich Art. 33) nicht belegt.",
    )]);

    // ---- 8. Argumentationskette -----------------------------------------
    h1(&mut d, "8. Vorgeschlagene Argumentationskette für die Replik");
    for (i, s) in [
        vec![t("Der API-Beschrieb ist eine Information nach § 3 Abs. 2 und § 20 IDG ("), lk("VB.2021.00858", v858), t(").")],
        vec![t("Es gilt eine Vermutung für den Zugang; die Beweislast trägt das Steueramt ("), lk("BVGer A-4595/2020", b4595), t(").")],
        vec![t("Geheimhaltungsgründe sind konkret zu begründen, pauschale Sicherheits- oder Geheimnisargumente genügen nicht ("), lk("BGer 1C_136/2025", bz), t("; "), lk("VB.2019.00603", v603), t(").")],
        vec![t("Widerspruch: «vertraulich», aber gegen Business-Plan an Treuhand-Softwarelieferanten ausgehändigt (Rekurs, Abschnitt 3).")],
        vec![t("Vergleich: "), lk("swissdamed", smt), t(" und "), lk("EUDAMED", eut), t(" veröffentlichen ihre M2M-Schnittstellen ohne Login – ohne Sicherheitsproblem.")],
        vec![t("Rechtsvergleich: "), lk("TA Paris 2016", tap), t(" und "), lk("CdS 2270/2019", cds), t(".")],
    ]
    .into_iter()
    .enumerate()
    {
        let mut spans = s;
        spans.insert(0, t(&format!("{}.  ", i + 1)));
        d.para(BODY, LEAD, 5.0, false, 10.0, spans);
    }

    // ---- 9. Offene Punkte -----------------------------------------------
    h1(&mut d, "9. Offene Punkte vor dem Einreichen");
    bullet(&mut d, vec![t("Prüfen, ob die Zuger Vorinstanz nach der Rückweisung (1C_136/2025) neu entschieden hat.")]);
    bullet(&mut d, vec![t("Alle Schweizer Entscheide im Volltext auf die zitierten Erwägungen prüfen.")]);
    bullet(&mut d, vec![t("TA Paris 2016 und CdS 2270/2019 im Original prüfen (bisher Auszug bzw. Sekundärquellen).")]);
    bullet(&mut d, vec![t("swissdamed- und EUDAMED-Seiten zum Einreichungsdatum als Beilage sichern (PDF-Ausdruck mit Datum).")]);

    // ---- Quellen --------------------------------------------------------
    d.gap(8.0);
    d.rule();
    d.para(10.5, 14.0, 6.0, true, 0.0, vec![t("Quellen")]);
    let quellen = [
        ("BGer 1C_136/2025 vom 2. Februar 2026 (Kanton Zug, IT-Dokumente) – Volltext", U_BGER_ZUG),
        ("BGer zum Kanton Zug – Zuger Woche, 11.03.2026", U_ZUG_ZW),
        ("BGer zum Kanton Zug – Bote der Urschweiz, 04.03.2026", U_ZUG_BOTE),
        ("BGer zum Kanton Zug – Zuger Zeitung", U_ZUG_ZZ),
        ("BGer zum Kanton Zug – zentralplus", U_ZUG_ZP),
        ("BGE 142 II 313 (1C_296/2015)", U_BGE142),
        ("BVGer A-4595/2020", U_BVGER4595),
        ("BVGer A-1432/2016 (zitiert BGer 1C_509/2016)", U_BVGER1432),
        ("BVGer A-7405/2014", U_BVGER7405),
        ("VGr ZH VB.2019.00603", U_VB2019_603),
        ("VGr ZH VB.2021.00858", U_VB2021_858),
        ("VGr ZH VB.2024.00043", U_VB2024_43),
        ("VGr ZH VB.2022.00142", U_VB2022_142),
        ("Besprechung BGer 1C_272/2022 (Steuergeheimnis)", U_STEUERGEH),
        ("Kanton Luzern – Öffentlichkeitsprinzip", U_LU),
        ("Bundeskanzlei – FAQ Open-Source-Software", U_BK_OSS),
        ("BACS – Open-Source-Strategie (EMBAG Art. 9)", U_BACS_OSS),
        ("Swissmedic – swissdamed Technische Dokumentation", U_SM_TECH),
        ("Swissmedic – swissdamed M2M REST API Documentation", U_SM_API),
        ("Swissmedic – UDI (Art. 17 MepV)", U_SM_UDI),
        ("Walder Wyss – Publication of Technical Documentation for M2M in swissdamed", U_WW),
        ("EUDAMED – Technical documentation", U_EUD_TECH),
        ("EUDAMED – M2M User Guide", U_EUD_M2M),
        ("TA Paris, 10.03.2016, Nr. 1508951/5-2", U_TA_PARIS),
        ("Next INpact – Code source = document administratif", U_TA_PRESSE),
        ("CADA-Gutachten Nr. 20144578", U_CADA),
        ("Consiglio di Stato Nr. 2270/2019 – Agenda Digitale", U_CDS_2270),
        ("Consiglio di Stato – Software und Herstellerrechte – Agenda Digitale", U_CDS_PROD),
        ("FG Bremen 2 K 12/14", U_FG_BREMEN),
        ("Bundestag – Kleine Anfrage 19/1058 (ELSTER)", U_BT_ELSTER),
        ("BVerfG 2 BvC 3/07", U_BVERFG),
        ("VfGH V 85/11", U_VFGH),
    ];
    for (label, url) in quellen {
        let i = d.url(url);
        d.para(9.0, 12.0, 1.0, false, 0.0, vec![t("•  "), t(label)]);
        d.para(8.0, 11.0, 5.0, false, 14.0, vec![lk(url, i)]);
    }

    d
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        format!(
            "{}/docs/rekurs/Recherche_Rechtsprechung_API-Beschrieb_2026-10-30.pdf",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    let n = save(
        &findings(),
        &out,
        "Zugang zum API-Beschrieb – Rechtsprechung und Vergleich swissdamed/EUDAMED",
        "taxtsueri",
        true,
    );
    println!("OK -> {out}  ({n} Seite[n])");
}
