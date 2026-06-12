//! taxtsueri Desktop-GUI (eframe/egui, Win/Mac/Linux).
//!
//! UBS-Vermögensausweis (PDF) wählen → validierungsfähiges **eCH-0119-XML**
//! erstellen und speichern (importierbar in die Steuersoftware). Inklusive
//! GitHub-Releases-Update-Check (analog movement_logger_desktop).
//!
//! Benötigt zur Laufzeit `pdftotext` (poppler) zum Lesen des Vermögensausweises.

use eframe::egui;
use egui_file_dialog::FileDialog;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use taxtsueri::{dataset, document_to_xml, mt940, settings, update, vermoegensausweis};

/// Was der gerade offene Dateidialog bezweckt.
#[derive(PartialEq)]
enum Pending {
    None,
    OpenMt940,
    OpenVermoegen,
    Save,
    SavePseudo,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Logo (eingebettet); oben rechts in der App + als OS-Fenster-Icon.
const LOGO_PNG: &[u8] = include_bytes!("../../assets/logo-256.png");

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([760.0, 560.0])
        .with_title(format!("taxtsueri – Vermögensausweis → eCH-0119  (v{VERSION})"));
    if let Some(icon) = os_window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "taxtsueri",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

struct App {
    status: String,
    xml: Option<String>,
    /// Pseudo-Jahresrechnung als PDF (Bilanz + ER) — sofern ein MT940 geladen ist.
    pseudo_pdf: Option<Vec<u8>>,
    securities: usize,
    update_rx: Receiver<Option<update::UpdateInfo>>,
    update_info: Option<update::UpdateInfo>,
    /// Rein in egui gezeichneter Dateidialog (kein GTK/Portal/Thread).
    file_dialog: FileDialog,
    /// Wozu der offene Dialog dient (MT940/Vermögensausweis öffnen vs. Speichern).
    pending: Pending,
    /// Gewähltes MT940-File (Basis der Erklärung).
    mt940_path: Option<PathBuf>,
    /// Gewählter Vermögensausweis (PDF, Wertschriften).
    vermoegen_path: Option<PathBuf>,
    /// Verdächtige Buchungen, die im Prüf-Pop-Up bestätigt werden müssen
    /// (Some = Fenster offen).
    review: Option<Vec<mt940::Flag>>,
    /// GPU-Textur fürs Logo oben rechts (lazy hochgeladen).
    logo_tex: Option<egui::TextureHandle>,
}

impl App {
    fn new() -> Self {
        // Update-Check im Hintergrund (blockiert die UI nicht).
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(update::check_latest(VERSION));
        });
        Self {
            status: "Bereit. Wähle einen UBS-Vermögensausweis (PDF).".into(),
            xml: None,
            pseudo_pdf: None,
            securities: 0,
            update_rx: rx,
            update_info: None,
            file_dialog: FileDialog::new()
                .add_file_filter(
                    "PDF",
                    std::sync::Arc::new(|p| p.extension().is_some_and(|e| e == "pdf")),
                )
                .default_file_name("steuererklaerung-2025.xml"),
            pending: Pending::None,
            mt940_path: None,
            vermoegen_path: None,
            review: None,
            logo_tex: None,
        }
    }

    /// Schritt 3: prüft verdächtige MT940-Buchungen. Gibt es welche, öffnet sich das
    /// Prüf-Pop-Up (`review`); sonst wird direkt generiert.
    fn start_generate(&mut self) {
        if let Some(p) = self.mt940_path.clone() {
            if let Ok(b) = std::fs::read(&p) {
                if let Ok(stmt) = mt940::parse(&String::from_utf8_lossy(&b)) {
                    let flags = mt940::flagged_credits(&stmt);
                    if !flags.is_empty() {
                        self.review = Some(flags);
                        return;
                    }
                }
            }
        }
        self.do_generate();
    }

    /// Kombiniert MT940-Konto (Basis) + Vermögensausweis (Wertschriften) zu einem
    /// eCH-0119-Wertschriftenverzeichnis und erzeugt das XML.
    fn do_generate(&mut self) {
        let mut entries: Vec<taxtsueri::model::SecurityEntry> = Vec::new();
        let mut parts: Vec<String> = Vec::new();
        let mut mt940_stmt: Option<mt940::Statement> = None;
        let mut wertschriften_cents: Option<i64> = None;
        self.pseudo_pdf = None;

        // MT940-Konto als Basis (Schlusssaldo = Vermögen, Zinsen = Ertrag).
        if let Some(p) = self.mt940_path.clone() {
            match std::fs::read(&p) {
                Ok(b) => match mt940::parse(&String::from_utf8_lossy(&b)) {
                    Ok(stmt) => {
                        entries.push(mt940::account_security_entry(&stmt));
                        parts.push(format!("MT940-Konto {}", stmt.account));
                        mt940_stmt = Some(stmt);
                    }
                    Err(e) => {
                        self.status = format!("MT940 nicht lesbar: {e}");
                        return;
                    }
                },
                Err(e) => {
                    self.status = format!("MT940 nicht lesbar: {e}");
                    return;
                }
            }
        }

        // Vermögensausweis (PDF) → Wertschriften.
        if let Some(p) = self.vermoegen_path.clone() {
            match run_pdftotext(&p) {
                Ok(text) => {
                    let secs = vermoegensausweis::to_securities(&vermoegensausweis::parse_text(&text));
                    let chf: i64 = secs.iter().filter_map(|s| s.tax_value.map(|t| t.cantonal)).sum();
                    wertschriften_cents = Some(chf * 100);
                    parts.push(format!("{} Wertschriften", secs.len()));
                    entries.extend(secs);
                }
                Err(e) => {
                    self.status = format!("PDF konnte nicht gelesen werden: {e}");
                    return;
                }
            }
        }

        // Pseudo-Jahresrechnung (Bilanz + ER) als PDF, sobald ein MT940 vorliegt.
        if let Some(stmt) = &mt940_stmt {
            let ps = mt940::pseudo_statements(stmt, wertschriften_cents);
            self.pseudo_pdf = Some(taxtsueri::pdf_report::pseudo_statements_pdf(&ps));
        }

        if entries.is_empty() {
            self.status = "Bitte mindestens ein MT940-File oder einen Vermögensausweis wählen.".into();
            return;
        }

        let los = vermoegensausweis::build_list_of_securities(entries);
        let n = los.security_entry.len();
        // Beispiel-Basis (Person/Kopf) + AHVN13 aus settings.json; Wertschriften ersetzen.
        let mut doc = dataset::example();
        if let Some(vn) = settings::load().np.vn {
            doc.content.main_form.person_data_partner1.identification.vn = vn;
        }
        doc.content.list_of_securities = Some(los);
        match document_to_xml(doc) {
            Ok(xml) => {
                self.securities = n;
                self.status = format!("{} → eCH-0119-XML erstellt ({n} Positionen).", parts.join(" + "));
                self.xml = Some(xml);
            }
            Err(e) => self.status = format!("Fehler beim Serialisieren: {e}"),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(info) = self.update_rx.try_recv() {
            self.update_info = info;
        }
        // Dateidialog (in-app) zeichnen und ein eben gewähltes Resultat abholen.
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_selected() {
            match self.pending {
                Pending::OpenMt940 => self.mt940_path = Some(path),
                Pending::OpenVermoegen => self.vermoegen_path = Some(path),
                Pending::SavePseudo => {
                    if let Some(pdf) = &self.pseudo_pdf {
                        self.status = match std::fs::write(&path, pdf) {
                            Ok(()) => format!("Pseudo-Jahresrechnung gespeichert: {}", path.display()),
                            Err(e) => format!("Speichern fehlgeschlagen: {e}"),
                        };
                    }
                }
                Pending::Save => {
                    if let Some(xml) = &self.xml {
                        self.status = match std::fs::write(&path, xml) {
                            Ok(()) => format!("Gespeichert: {}", path.display()),
                            Err(e) => format!("Speichern fehlgeschlagen: {e}"),
                        };
                    }
                }
                Pending::None => {}
            }
            self.pending = Pending::None;
        }
        // Logo-Textur lazy hochladen.
        if self.logo_tex.is_none() {
            self.logo_tex = decode_logo()
                .map(|img| ctx.load_texture("taxtsueri-logo", img, egui::TextureOptions::LINEAR));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("Steuererklärung Zürich — MT940-Konto + Vermögensausweis");
                    ui.label("MT940 (Kontoauszug, Basis) + UBS-Vermögensausweis (PDF) → validierungsfähiges eCH-0119-XML.");
                });
                // Logo rechts in die Ecke verankern (klickbar → mailto), wie
                // beim MovementLogger.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if let Some(tex) = self.logo_tex.as_ref() {
                        let size = egui::vec2(56.0, 56.0);
                        let resp = ui
                            .add(egui::ImageButton::new((tex.id(), size)).frame(false))
                            .on_hover_text("E-Mail an zdavatz@ywesee.com")
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.clicked() {
                            ui.ctx()
                                .open_url(egui::OpenUrl::new_tab("mailto:zdavatz@ywesee.com"));
                        }
                    }
                });
            });

            if let Some(u) = &self.update_info {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(190, 120, 0),
                        format!("Update verfügbar: {} (läuft: v{VERSION})", u.pretty()),
                    );
                    if ui.button("Release öffnen").clicked() {
                        open_url(&u.url);
                    }
                });
            }

            ui.separator();
            let name = |p: &Option<PathBuf>| {
                p.as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "—".into())
            };
            // Klarer 3-Schritt-Ablauf, untereinander.
            ui.horizontal(|ui| {
                if ui.button("1.  🏦  MT940-Kontoauszug wählen …").clicked() {
                    self.pending = Pending::OpenMt940;
                    self.file_dialog.select_file();
                }
                ui.label(format!("→ {}", name(&self.mt940_path)));
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui.button("2.  📄  Vermögensausweis wählen … (PDF)").clicked() {
                    self.pending = Pending::OpenVermoegen;
                    self.file_dialog.select_file();
                }
                ui.label(format!("→ {}", name(&self.vermoegen_path)));
            });
            ui.add_space(6.0);
            let ready = self.mt940_path.is_some() || self.vermoegen_path.is_some();
            if ui
                .add_enabled(ready, egui::Button::new("3.  ⚙  Steuererklärung generieren"))
                .clicked()
            {
                self.start_generate();
            }
            ui.add_space(4.0);
            ui.label(&self.status);

            // Pseudo-Jahresrechnung (Bilanz + ER) als PDF — sobald ein MT940 geladen ist.
            if self.pseudo_pdf.is_some()
                && ui
                    .button("📊  Pseudo-Jahresrechnung (PDF) speichern …  (Entwurf)")
                    .on_hover_text("Cash-Basis-Entwurf: S.1 Bilanz (inkl. Wertschriften), S.2 Erfolgsrechnung — zur Prüfung durch den Vermögensverwalter")
                    .clicked()
            {
                self.pending = Pending::SavePseudo;
                self.file_dialog.save_file();
            }

            if let Some(xml) = self.xml.clone() {
                ui.separator();
                ui.label(format!("{} Positionen · {} Bytes XML", self.securities, xml.len()));
                if ui.button("💾  eCH-0119-XML speichern …").clicked() {
                    self.pending = Pending::Save;
                    self.file_dialog.save_file();
                }
                ui.add_space(4.0);
                // Textfarbe ans Theme anpassen: hell auf dunkel, dunkel auf
                // hell — sonst ist das XML im Dark Mode kaum lesbar.
                let xml_color = if ui.visuals().dark_mode {
                    egui::Color32::from_gray(230)
                } else {
                    egui::Color32::from_gray(20)
                };
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&xml).monospace().color(xml_color));
                    });
            }
        });

        // Prüf-Pop-Up: verdächtige Buchungen vor dem Generieren bestätigen.
        if self.review.is_some() {
            let flags = self.review.clone().unwrap();
            let (mut proceed, mut cancel) = (false, false);
            egui::Window::new("⚠  Verdächtige Buchungen prüfen")
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Das Steueramt prüft grössere Gutschriften, die nicht als Zins erkannt \
                         wurden. Ist das steuerbares Einkommen (Lohn/Honorar/ausländischer Ertrag) \
                         oder ein nicht-steuerbarer Eigenübertrag? Die Software bucht diese NICHT \
                         automatisch als Ertrag.",
                    );
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for f in &flags {
                            ui.group(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}   CHF {}",
                                        f.date,
                                        mt940::format_cents(f.amount_cents)
                                    ))
                                    .strong(),
                                );
                                if !f.description.is_empty() {
                                    ui.label(&f.description);
                                }
                                ui.label(
                                    egui::RichText::new(format!("Kategorie: {}", f.category))
                                        .weak(),
                                );
                            });
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button("Geprüft – generieren (nur Zins als Ertrag)")
                            .clicked()
                        {
                            proceed = true;
                        }
                        if ui.button("Abbrechen").clicked() {
                            cancel = true;
                        }
                    });
                });
            if proceed {
                self.review = None;
                self.do_generate();
            } else if cancel {
                self.review = None;
                self.status = "Abgebrochen — bitte verdächtige Buchungen prüfen.".into();
            }
        }
    }
}

/// Dekodiert das Logo-PNG fürs In-App-Anzeigen (egui-ColorImage).
fn decode_logo() -> Option<egui::ColorImage> {
    let img = image::load_from_memory(LOGO_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        img.as_raw(),
    ))
}

/// Dekodiert das Logo fürs OS-Fenster-Icon (Dock/Taskbar).
fn os_window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(LOGO_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

/// Liest den Text eines (Text-)PDFs via `pdftotext -layout`.
fn run_pdftotext(path: &Path) -> Result<String, String> {
    let out = Command::new("pdftotext")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .map_err(|_| "pdftotext nicht gefunden (poppler-utils installieren)".to_string())?;
    if !out.status.success() {
        return Err("pdftotext fehlgeschlagen".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Öffnet eine URL im Standardbrowser (plattformabhängig).
fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}
