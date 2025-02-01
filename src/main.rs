use eframe::egui;
use eframe::Frame;
use rfd::FileDialog;
use std::fs;
use std::path::Path;

struct FolderLetterEntry {
    folder: String,
    letter: char,
}

struct MyApp {
    selected_folder: Option<String>,
    image_paths: Vec<String>,
    current_image_index: usize, // Keep track of the currently displayed image
    folder_letter_entries: Vec<FolderLetterEntry>,
    new_folder: String,
    new_letter: String,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            selected_folder: None,
            image_paths: Vec::new(),
            current_image_index: 0, // Start with the first image (index 0)
            folder_letter_entries: Vec::new(),
            new_folder: String::new(),
            new_letter: String::new(),
        }
    }
}

fn get_image_paths(folder_path: &str) -> Vec<String> {
    let mut image_paths = Vec::new();
    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        let ext_lower = ext_str.to_lowercase(); // Case-insensitive check
                        if ext_lower == "jpg"
                            || ext_lower == "jpeg"
                            || ext_lower == "png"
                            || ext_lower == "gif"
                            || ext_lower == "webp"
                        {
                            // Add more extensions as needed
                            if let Some(path_str) = path.to_str() {
                                image_paths.push(path_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    image_paths
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::J) {
                if !self.image_paths.is_empty() {
                    self.current_image_index =
                        (self.current_image_index + 1) % self.image_paths.len();
                }
            }
            if i.key_pressed(egui::Key::K) {
                if !self.image_paths.is_empty() {
                    self.current_image_index =
                        self.current_image_index.wrapping_sub(1) % self.image_paths.len();
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                let available_height = ui.available_size().y;
                let image_height = available_height * 0.7;

                if ui.button("Choose Folder").clicked() {
                    if let Some(path) = FileDialog::new().pick_folder() {
                        self.selected_folder = Some(path.to_string_lossy().to_string());
                        if let Some(folder) = &self.selected_folder {
                            self.image_paths = get_image_paths(folder); // Update image paths
                        }
                    }
                }

                ui.label("Selected Folder:");
                match &self.selected_folder {
                    Some(folder) => ui.label(folder),
                    None => ui.label("No folder selected."),
                };

                let available_height = ui.available_size().y;
                // Limit upper part to 70%.
                let image_height = available_height * 0.7;
                let image_area = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(ui.available_width(), image_height),
                );

                // Display the current image:
                if let Some(path) = self.image_paths.get(self.current_image_index) {
                    ui.label(format!("Current Image: {}", path));
                    let path = format!("file://{}", path);
                    ui.add(
                        egui::Image::new(egui::ImageSource::Uri(std::borrow::Cow::from(path)))
                            .fit_to_exact_size(image_area.size()),
                    );
                } else if !self.image_paths.is_empty() {
                    ui.label("No images found in the folder.");
                } else {
                    ui.label("No folder selected.");
                }

                ui.separator();

                // Control area.
                ui.vertical(|ui| {
                    ui.label("Folder & Letter Entries:");

                    let available_height = ui.available_size().y;
                    let control_height = available_height * 0.3;
                    egui::ScrollArea::vertical()
                        .min_scrolled_height(control_height)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Folder:");
                                if ui.button("Choose Folder").clicked() {
                                    // Button to open file dialog
                                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                        self.new_folder = path.to_string_lossy().to_string();
                                        // Update the new_folder string
                                    }
                                }
                                ui.text_edit_singleline(&mut self.new_folder); // Display the chosen path

                                ui.label("Letter:");
                                ui.text_edit_singleline(&mut self.new_letter);

                                if ui.button("+").clicked() && !self.new_folder.is_empty() {
                                    if let Some(letter) = self.new_letter.chars().next() {
                                        self.folder_letter_entries.push(FolderLetterEntry {
                                            folder: self.new_folder.clone(),
                                            letter,
                                        });
                                        self.new_folder.clear();
                                        self.new_letter.clear();
                                    }
                                }
                            });

                            // Display Folder & Letter Entries:
                            for entry in &self.folder_letter_entries {
                                ui.label(format!(
                                    "Folder: {}, Letter: {}",
                                    entry.folder, entry.letter
                                ));
                            }
                        });
                });
            })
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Image organizer",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(MyApp::default()))
        }),
    )
}
