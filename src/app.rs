mod api;
use crate::app::api::Release;

use anyhow::Result;
use eframe::{egui, egui::*, epi};
use std::fs;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct ProtonGEManager {
    filter_query: String,
    metadata: Vec<Release>,
    selected_release: Option<Release>,
    send: Sender<Result<Message>>,
    recv: Receiver<Result<Message>>,
    install_status: InstallStatus,
}

enum InstallStatus {
    Ready,
    Downloading,
    Installing,
    Error(String),
}

enum Message {
    StartInstall(String),
    InstallSuccess,
}

impl Default for ProtonGEManager {
    fn default() -> Self {
        let (send, recv) = channel();
        Self {
            filter_query: String::new(),

            metadata: Release::fetch_metadata(),
            selected_release: None,
            send,
            recv,
            install_status: InstallStatus::Ready,
        }
    }
}

impl epi::App for ProtonGEManager {
    fn name(&self) -> &str {
        "Proton-GE Manager"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        let Self {
            filter_query,
            metadata,
            selected_release,
            send,
            recv,
            install_status,
        } = self;

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Releases");

            ui.horizontal(|ui| {
                ui.label("Filter: ");
                ui.text_edit_singleline(filter_query);
            });

            let scroll_area = ScrollArea::vertical();

            ui.separator();

            let (_current_scroll, _max_scroll) = scroll_area.show(ui, |ui| {
                ui.vertical(|ui| {
                    for release in metadata {
                        if ui.selectable_label(true, &release.tag_name).clicked() {
                            *selected_release = Some(release.clone());
                        };
                    }
                });

                let margin = ui.visuals().clip_rect_margin;

                let current_scroll = ui.clip_rect().top() - ui.min_rect().top() + margin;
                let max_scroll = ui.min_rect().height() - ui.clip_rect().height() + 2.0 * margin;
                (current_scroll, max_scroll)
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Release notes");

            if let Some(selected_release) = &mut self.selected_release {
                ui.separator();

                let scroll_area = ScrollArea::vertical();
                let (_current_scroll, _max_scroll) = scroll_area.show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.hyperlink(&selected_release.html_url);
                        ui.horizontal(|ui| {
                            if ui.button("Install").clicked() {
                                let assets = selected_release.assets.clone();

                                for asset in assets {
                                    let send = send.clone();
                                    if asset.content_type.eq("application/gzip") {
                                        std::thread::spawn(move || {
                                            send.send(download(
                                                &asset.name.to_owned(),
                                                &asset.browser_download_url.to_owned(),
                                            ))
                                        });
                                    }
                                }
                                *install_status = InstallStatus::Downloading;
                            };

                            match install_status {
                                InstallStatus::Ready => ui.label("Ready"),
                                InstallStatus::Downloading => ui.label("Downloading"),
                                InstallStatus::Installing => ui.label("Installing"),
                                InstallStatus::Error(e) => ui.label(e),
                            };

                            if let Ok(res) = recv.try_recv() {
                                match res {
                                    Ok(Message::StartInstall(filename)) => {
                                        let send = send.clone();
                                        std::thread::spawn(move || {
                                            send.send(install(&filename.to_owned()))
                                        });
                                        *install_status = InstallStatus::Installing;
                                    }
                                    Ok(Message::InstallSuccess) => {
                                        *install_status = InstallStatus::Ready
                                    }
                                    Err(e) => {
                                        *install_status = InstallStatus::Error(e.to_string());
                                    }
                                };
                            };
                        });

                        ui.separator();

                        ui.label(&selected_release.body);
                    });

                    let margin = ui.visuals().clip_rect_margin;

                    let current_scroll = ui.clip_rect().top() - ui.min_rect().top() + margin;
                    let max_scroll =
                        ui.min_rect().height() - ui.clip_rect().height() + 2.0 * margin;
                    (current_scroll, max_scroll)
                });
            } else {
                ui.label("Select a release in the left panel to get started.");
            };
        });
    }
}

fn download(filename: &str, url: &str) -> Result<Message> {
    let filename = format!("/tmp/{}", filename);

    let mut res = reqwest::blocking::get(url)?;
    let mut buf: Vec<u8> = vec![];
    res.copy_to(&mut buf).unwrap();

    fs::write(&filename, buf)?;

    Ok(Message::StartInstall(filename))
}

fn install(filename: &str) -> Result<Message> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use tar::Archive;

    let home_dir = dirs::home_dir().unwrap();
    let install_dir = format!(
        "{}/.steam/root/compatibilitytools.d/",
        home_dir.to_str().unwrap()
    );
    fs::create_dir_all(&install_dir)?;

    let tar_gz = File::open(filename)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(install_dir)?;

    fs::remove_file(filename)?;

    Ok(Message::InstallSuccess)
}
