//! taxtsueri Desktop-GUI (eframe/egui, Win/Mac/Linux).
//!
//! UBS-Vermögensausweis (PDF) wählen → validierungsfähiges **eCH-0119-XML**
//! erstellen und speichern (importierbar in die Steuersoftware). Inklusive
//! GitHub-Releases-Update-Check (analog movement_logger_desktop).
//!
//! Benötigt zur Laufzeit `pdftotext` (poppler) zum Lesen des Vermögensausweises.

use eframe::egui;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use taxtsueri::{dataset, document_to_xml, settings, update, vermoegensausweis};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 560.0])
            .with_title(format!("taxtsueri – Vermögensausweis → eCH-0119  (v{VERSION})")),
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
    securities: usize,
    update_rx: Receiver<Option<update::UpdateInfo>>,
    update_info: Option<update::UpdateInfo>,
    /// Vom Datei-Auswahl-Thread gelieferter Eingabe-Pfad (PDF).
    open_rx: Option<Receiver<Option<PathBuf>>>,
    /// Vom Speichern-Thread gelieferte Status-Meldung.
    save_rx: Option<Receiver<String>>,
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
            securities: 0,
            update_rx: rx,
            update_info: None,
            open_rx: None,
            save_rx: None,
        }
    }

    fn generate(&mut self, path: &Path) {
        let text = match run_pdftotext(path) {
            Ok(t) => t,
            Err(e) => {
                self.status = format!("PDF konnte nicht gelesen werden: {e}");
                return;
            }
        };
        let los = vermoegensausweis::list_of_securities_from_text(&text);
        let n = los.security_entry.len();
        if n == 0 {
            self.status =
                "Keine Wertschriftenpositionen erkannt — ist das ein UBS-Vermögensausweis (Text-PDF)?".into();
            return;
        }
        // Beispiel-Basis (Person/Kopf) + AHVN13 aus settings.json; Wertschriften ersetzen.
        let mut doc = dataset::example();
        if let Some(vn) = settings::load().np.vn {
            doc.content.main_form.person_data_partner1.identification.vn = vn;
        }
        doc.content.list_of_securities = Some(los);
        match document_to_xml(doc) {
            Ok(xml) => {
                self.securities = n;
                self.status = format!("{n} Wertschriftenpositionen → eCH-0119-XML erstellt.");
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
        // Ergebnis des Datei-Auswahl-Threads abholen (rfd läuft NICHT auf dem
        // UI-Thread — das vermeidet GTK-Main-Loop-Reentrancy-Crashes).
        if let Some(rx) = &self.open_rx {
            if let Ok(picked) = rx.try_recv() {
                self.open_rx = None;
                if let Some(path) = picked {
                    self.generate(&path);
                }
            }
        }
        // Ergebnis des Speichern-Threads abholen.
        if let Some(rx) = &self.save_rx {
            if let Ok(msg) = rx.try_recv() {
                self.save_rx = None;
                self.status = msg;
            }
        }
        // Solange ein Dialog-Thread läuft, weiter neu zeichnen (sonst pollt
        // egui die Channels erst beim nächsten Eingabe-Event).
        if self.open_rx.is_some() || self.save_rx.is_some() {
            ctx.request_repaint();
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Steuererklärung Zürich — Wertschriften aus Vermögensausweis");
            ui.label("UBS-Vermögensausweis (PDF) → validierungsfähiges eCH-0119-XML für die Steuersoftware.");

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
            let busy = self.open_rx.is_some();
            if ui
                .add_enabled(!busy, egui::Button::new("📄  UBS-Vermögensausweis wählen … (PDF)"))
                .clicked()
            {
                // Dialog auf eigenem Thread öffnen; Ergebnis per Channel zurück.
                let (tx, rx) = channel();
                self.open_rx = Some(rx);
                self.status = "Dateiauswahl geöffnet …".into();
                std::thread::spawn(move || {
                    let picked = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file();
                    let _ = tx.send(picked);
                });
            }
            ui.add_space(4.0);
            ui.label(&self.status);

            if let Some(xml) = self.xml.clone() {
                ui.separator();
                ui.label(format!("{} Positionen · {} Bytes XML", self.securities, xml.len()));
                let saving = self.save_rx.is_some();
                if ui
                    .add_enabled(!saving, egui::Button::new("💾  eCH-0119-XML speichern …"))
                    .clicked()
                {
                    // Speichern-Dialog ebenfalls auf eigenem Thread.
                    let (tx, rx) = channel();
                    self.save_rx = Some(rx);
                    let xml = xml.clone();
                    std::thread::spawn(move || {
                        let msg = match rfd::FileDialog::new()
                            .set_file_name("steuererklaerung-2025.xml")
                            .add_filter("XML", &["xml"])
                            .save_file()
                        {
                            Some(path) => match std::fs::write(&path, &xml) {
                                Ok(()) => format!("Gespeichert: {}", path.display()),
                                Err(e) => format!("Speichern fehlgeschlagen: {e}"),
                            },
                            None => "Speichern abgebrochen.".into(),
                        };
                        let _ = tx.send(msg);
                    });
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    ui.monospace(&xml);
                });
            }
        });
    }
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
