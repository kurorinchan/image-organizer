use eframe::egui;
use egui::{FontData, FontDefinitions, FontFamily};
use rfd::FileDialog;
use std::{fs, path::Path, path::PathBuf};

use anyhow::{bail, Result};

#[derive(Clone, Debug)]
struct FolderLetterEntry {
    folder: String,
    letter: char,
}

#[derive(Clone, Debug, Default)]
struct MoveLogEntry {
    // Original source file path.
    src: String,
    // Where the file was moved. Full path (i.e. not just destination dir).
    dest: String,
}

#[derive(Default)]
struct MyApp {
    selected_folder: Option<String>,
    image_paths: Vec<String>,
    current_image_index: usize, // Keep track of the currently displayed image
    folder_letter_entries: Vec<FolderLetterEntry>,
    new_folder: String,
    new_letter: String,
    move_log: Vec<MoveLogEntry>,
    status_message: String,
}

fn get_image_paths(folder_path: &str) -> Vec<String> {
    let mut image_paths = Vec::new();
    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Some(extension) = path.extension() else {
                continue;
            };
            let Some(ext_str) = extension.to_str() else {
                continue;
            };

            let ext_lower = ext_str.to_lowercase();
            // TODO: There is also image::ImageFormat.all() and then call can_read() to see if
            // the current features allow reading the file. Then use extension_str() to get
            // all the extensions for that image format.
            let image_extensions = ["jpg", "jpeg", "png", "gif", "webp"];
            if image_extensions.contains(&ext_lower.as_str()) {
                // Add more extensions as needed
                if let Some(path_str) = path.to_str() {
                    image_paths.push(path_str.to_string());
                }
            }
        }
    }

    // It's likely that screenshot names are named by date it was taken. Sorting
    // and reversing it would show the latest images first.
    image_paths.sort();
    image_paths.reverse();
    image_paths
}

fn get_file_name(path: &str) -> String {
    let path = Path::new(path);
    path.file_name().unwrap().to_string_lossy().to_string()
}

// Moves src to dest_dir. Returns the new file path on success.
fn move_file(src: &str, dest_dir: &str) -> std::io::Result<String> {
    let src_path = Path::new(src);
    let filename = src_path.file_name().unwrap();
    let dest_path = PathBuf::from(dest_dir).join(filename);
    std::fs::rename(src, &dest_path)?;
    Ok(dest_path.to_string_lossy().to_string())
}

impl MyApp {
    fn move_current_image_to_dest(&mut self, dest_dir: &str) -> Result<MoveLogEntry> {
        let Some(image_path) = self.image_paths.get(self.current_image_index) else {
            bail!(
                "No image path found at the current index {}.",
                self.current_image_index
            );
        };

        let image_path = image_path.clone();
        match move_file(&image_path, dest_dir) {
            Ok(new_path) => {
                log::info!("Moved file {} to {}", image_path, dest_dir);
                self.image_paths.remove(self.current_image_index);
                // Handle the case where the current_image_index is now out of
                // bounds because it (re)moved the last file.
                if self.current_image_index >= self.image_paths.len()
                    && self.current_image_index > 0
                {
                    self.current_image_index = self.image_paths.len() - 1;
                }
                let log_entry = MoveLogEntry {
                    src: image_path.clone(),
                    dest: new_path.clone(),
                };
                self.move_log.push(log_entry.clone());
                Ok(log_entry)
            }
            Err(e) => {
                log::error!("Failed to move file: {}", e);
                Err(e.into())
            }
        }
    }

    fn next_image(&mut self) {
        self.current_image_index = (self.current_image_index + 1) % self.image_paths.len();
    }

    fn previous_image(&mut self) {
        if self.current_image_index == 0 {
            self.current_image_index = self.image_paths.len() - 1;
        } else {
            self.current_image_index -= 1;
        }
    }

    fn remove_folder_letter_entries(&mut self, indecies: Vec<usize>) {
        let mut indecies = indecies;
        indecies.sort();
        indecies.reverse();
        for index in indecies {
            self.folder_letter_entries.remove(index);
        }
    }

    // Undo the last move. The image is reinserted to the current index.
    // Returns the path to the un-done file.
    fn undo_move(&mut self) -> Option<String> {
        if self.move_log.is_empty() {
            return None;
        }
        let last_move = self.move_log.pop().unwrap();
        let src = last_move.src;
        let dest = last_move.dest;
        std::fs::rename(&dest, &src).ok()?;
        self.image_paths
            .insert(self.current_image_index, src.clone());
        Some(src)
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut status_message = String::new();
        ctx.input(|input| {
            if input.key_pressed(egui::Key::J) {
                self.next_image();
            }
            if input.key_pressed(egui::Key::K) {
                self.previous_image();
            }

            if input.modifiers.ctrl && input.key_pressed(egui::Key::Z) {
                match self.undo_move() {
                    Some(_) => {
                        status_message = "Undo".to_string();
                    }
                    None => {
                        status_message = "Nothing to undo.".to_string();
                    }
                }
            }

            // If registered letter is pressed, move the file to the folder.
            for entry in self.folder_letter_entries.clone().iter() {
                let letter = entry.letter;
                let Some(key) = egui::Key::from_name(&letter.to_string()) else {
                    // TODO: This probably spams the log. Do it on register.
                    log::error!("Invalid folder letter: {}", letter);
                    continue;
                };
                if !input.key_pressed(key) {
                    continue;
                }

                let dest_dir = &entry.folder;
                match self.move_current_image_to_dest(dest_dir) {
                    Ok(move_log) => {
                        let filename = get_file_name(&move_log.src);
                        status_message = format!("Moved {} -> {}", filename, dest_dir);
                        log::info!("{}", &status_message);
                    }
                    Err(e) => {
                        status_message = format!("Failed to move file: {}", e);
                        log::error!("{}", &status_message);
                    }
                };
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("Choose Image Folder").clicked() {
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
                    ui.label(format!("({})", self.image_paths.len()));
                });

                ui.horizontal(|ui| {
                    ui.label("Status:");
                    if !status_message.is_empty() {
                        // The status message has to be copied because any update will set the
                        // cleared status message.
                        self.status_message = status_message;
                    }
                    ui.label(&self.status_message);
                });

                let available_height = ui.available_size().y;
                // Limit upper part to 70%.
                let image_height = available_height * 0.7;
                let image_area = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(ui.available_width(), image_height),
                );

                // Display the current image:
                if let Some(path) = self.image_paths.get(self.current_image_index) {
                    let filename = get_file_name(path);
                    let n_out_of_all = format!(
                        "({}/{})",
                        self.current_image_index + 1,
                        self.image_paths.len()
                    );
                    ui.label(format!("Current Image: {} {}", n_out_of_all, filename));
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
                                if ui.button("Choose Destination Folder").clicked() {
                                    // Button to open file dialog
                                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                        self.new_folder = path.to_string_lossy().to_string();
                                    }
                                }
                                ui.text_edit_singleline(&mut self.new_folder); // Display the chosen path

                                ui.label("Letter:");
                                ui.text_edit_singleline(&mut self.new_letter);
                            });

                            if ui.button("+").clicked()
                                && !self.new_folder.is_empty()
                                && !self.new_letter.is_empty()
                            {
                                if let Some(letter) = self.new_letter.chars().next() {
                                    self.folder_letter_entries.push(FolderLetterEntry {
                                        folder: self.new_folder.clone(),
                                        letter,
                                    });
                                    self.new_folder.clear();
                                    self.new_letter.clear();
                                }
                            }

                            let mut remove_index = vec![];
                            // Display Folder & Letter Entries:
                            for (index, entry) in self.folder_letter_entries.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "Folder: {}, Letter: {}",
                                        entry.folder, entry.letter
                                    ));
                                    if ui.button("X").clicked() {
                                        remove_index.push(index);
                                    }
                                });
                            }

                            self.remove_folder_letter_entries(remove_index);
                        });
                });
            })
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport =
        egui::ViewportBuilder::default().with_inner_size(egui::Vec2::new(1280.0, 960.0));

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        FontData::from_static(include_bytes!("../fonts/NotoSansJP-VariableFont_wght.ttf")).into(),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "my_font".to_owned());
    eframe::run_native(
        "Image organizer",
        native_options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(fonts);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(MyApp::default()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Move a temporary file from one folder to another.
    #[test]
    fn move_file_test() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("test.txt");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path, b"Hello, world!").unwrap();
        assert!(src_path.exists());
        move_file(&src_path.to_string_lossy(), &dest_dir.to_string_lossy()).unwrap();
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.txt").exists());
    }

    #[test]
    fn move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("test.txt");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path, b"Hello, world!").unwrap();
        app.image_paths = vec![src_path.to_string_lossy().to_string()];
        app.current_image_index = 0;
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.txt").exists());
    }

    // Given there are mulitple files in the src folder, move the current image to the dest folder.
    #[test]
    fn multiple_one_file_move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path1 = temp_dir.path().join("test1.txt");
        let src_path2 = temp_dir.path().join("test2.txt");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path1, b"Hello, world!").unwrap();
        std::fs::write(&src_path2, b"Hello, world!").unwrap();
        app.image_paths = vec![
            src_path1.to_string_lossy().to_string(),
            src_path2.to_string_lossy().to_string(),
        ];

        // Move the file at the first index. Make sure the second file is not affected.
        app.current_image_index = 0;
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        assert!(!src_path1.exists());
        assert!(src_path2.exists());
        assert!(dest_dir.join("test1.txt").exists());
        assert!(!dest_dir.join("test2.txt").exists());
    }

    #[test]
    fn move_all_files_move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path1 = temp_dir.path().join("test1.txt");
        let src_path2 = temp_dir.path().join("test2.txt");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path1, b"Hello, world!").unwrap();
        std::fs::write(&src_path2, b"Hello, world!").unwrap();
        app.image_paths = vec![
            src_path1.to_string_lossy().to_string(),
            src_path2.to_string_lossy().to_string(),
        ];
        app.current_image_index = 0;
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        assert!(!src_path1.exists());
        assert!(!src_path2.exists());
        assert!(dest_dir.join("test1.txt").exists());
        assert!(dest_dir.join("test2.txt").exists());
    }

    #[test]
    fn remove_folder_letter_entries_test() {
        let mut app = MyApp {
            folder_letter_entries: vec![
                FolderLetterEntry {
                    folder: "folder1".to_string(),
                    letter: 'A',
                },
                FolderLetterEntry {
                    folder: "folder2".to_string(),
                    letter: 'B',
                },
            ],
            ..Default::default()
        };
        let indecies = vec![0, 1];
        app.remove_folder_letter_entries(indecies);
        assert!(app.folder_letter_entries.is_empty());
    }

    #[test]
    fn undo_move_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("test.txt");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();

        std::fs::write(&src_path, b"Hello, world!").unwrap();
        app.image_paths = vec![src_path.to_string_lossy().to_string()];
        app.current_image_index = 0;
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();

        // Make sure its not in image paths anymore and has been moved.
        assert!(!app
            .image_paths
            .contains(&src_path.to_string_lossy().to_string()));
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.txt").exists());

        // Now undo and check that everything is rolled back.
        let Some(undo_path) = app.undo_move() else {
            panic!("undo_move() returned None");
        };
        assert_eq!(undo_path, src_path.to_string_lossy());
        assert!(src_path.exists());
        assert!(!dest_dir.join("test.txt").exists());
        assert!(app
            .image_paths
            .contains(&src_path.to_string_lossy().to_string()));

        // Further undo should return None.
        assert!(app.undo_move().is_none());
        assert!(app.undo_move().is_none());
    }
}
