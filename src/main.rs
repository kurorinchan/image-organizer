use eframe::egui;
use egui::{FontData, FontDefinitions, FontFamily, Vec2};
use rfd::FileDialog;
use rust_embed::Embed;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};

#[derive(Embed)]
#[folder = "fonts"]
struct FontAsset;

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

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
struct ImagePath {
    path: String,
}

impl ImagePath {
    fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn uri(&self) -> String {
        format!("file://{}", self.path)
    }
}

// This contains a list of images that are loaded in egui right now. Anything that is not properly
// unloaded is memory leak.
#[derive(Default)]
struct Loader {
    image_paths: HashSet<ImagePath>,
    context: egui::Context,
}

impl Loader {
    /// Set the current context.
    fn set_context(&mut self, context: &egui::Context) {
        self.context = context.clone();
    }

    /// Add a new image to be loaded.
    /// Actual loading happens when the image is added to the `ui` in `egui.`
    fn add(&mut self, path: &str) -> egui::Image {
        let image_path = ImagePath::new(path);
        if self.image_paths.insert(image_path.clone()) {
            log::info!(
                "Added image. Number of Loaded images: {}",
                self.image_paths.len()
            );
        }
        egui::Image::from_uri(image_path.uri())
    }

    /// Remove images from the loader except those specified in `paths`.
    fn only_keep(&mut self, paths: Vec<String>) {
        let new_set: HashSet<ImagePath> =
            HashSet::from_iter(paths.iter().map(|p| ImagePath::new(p)));
        let still_loaded = &self.image_paths - &new_set;
        if still_loaded.is_empty() {
            return;
        }
        for path in still_loaded {
            log::debug!("OnlyKeep: Removing image: {}", path.path());
            self.image_paths.remove(&path);
            self.context.forget_image(&path.uri());
        }
    }
}

#[derive(Default)]
struct ImageManager {
    all_images: Vec<String>,
    current_image_index: usize,
    loader: Loader,
}

struct LoadedImageInfo<'a> {
    path: String,
    image: egui::Image<'a>,
}

impl ImageManager {
    fn set_context(&mut self, context: &egui::Context) {
        self.loader.set_context(context);
    }

    fn set_image_folder(&mut self, folder_path: &str) {
        self.all_images = get_image_paths(folder_path);
        self.current_image_index = 0;
    }

    fn load_current_image(&mut self) -> Option<LoadedImageInfo> {
        let path = self.all_images.get(self.current_image_index);
        match path {
            Some(path) => Some(LoadedImageInfo {
                path: path.clone(),
                image: self.loader.add(path),
            }),
            None => None,
        }
    }

    // Only load images within 3 indices of the current image.
    fn cleanup(&mut self) {
        let start = std::cmp::max(0, self.current_image_index.saturating_sub(3));
        let end = std::cmp::min(
            self.all_images.len(),
            self.current_image_index.saturating_add(3),
        );
        let mut keep_images = Vec::new();
        for i in start..end {
            keep_images.push(self.all_images[i].to_string());
        }
        self.loader.only_keep(keep_images);
    }

    fn num_images(&self) -> usize {
        self.all_images.len()
    }

    fn current_index(&self) -> usize {
        self.current_image_index
    }

    fn next_image(&mut self) {
        self.current_image_index = (self.current_image_index + 1) % self.num_images();
    }

    fn previous_image(&mut self) {
        if self.current_image_index == 0 {
            self.current_image_index = self.num_images() - 1;
        } else {
            self.current_image_index -= 1;
        }
    }

    fn remove_current_image(&mut self) -> Option<String> {
        if self.current_image_index >= self.all_images.len() {
            log::error!(
                "Current image index is {} but only has {}.",
                self.current_image_index,
                self.all_images.len()
            );
            return None;
        }
        let path = self.all_images.remove(self.current_image_index);
        //self.loader.remove(&path);

        // Handle the case where the current_image_index is now out of bounds
        // because it (re)moved the last file.
        if self.current_image_index >= self.all_images.len() && self.current_image_index > 0 {
            self.current_image_index = self.all_images.len() - 1;
        }

        log::debug!(
            "Removed image. Current index {}. Number of images: {}",
            self.current_image_index,
            self.all_images.len()
        );

        Some(path)
    }

    fn add_image(&mut self, path: &str) {
        self.all_images
            .insert(self.current_image_index, path.to_string());
        self.loader.add(path);
    }
}

#[derive(Default)]
struct MyApp {
    selected_folder: Option<String>,
    folder_letter_entries: Vec<FolderLetterEntry>,
    new_folder: String,
    new_letter: String,
    move_log: Vec<MoveLogEntry>,
    status_message: String,
    image_manager: ImageManager,
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
        let Some(image_path) = self.image_manager.remove_current_image() else {
            bail!("Failed to find current image");
        };

        let image_path = image_path.clone();
        match move_file(&image_path, dest_dir) {
            Ok(new_path) => {
                log::info!("Moved file {} to {}", image_path, dest_dir);
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
        self.image_manager.next_image();
    }

    fn previous_image(&mut self) {
        self.image_manager.previous_image();
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
        self.image_manager.add_image(&src);
        Some(src)
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.image_manager.set_context(ctx);
        self.image_manager.cleanup();
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
                log::debug!(
                    "Pressed key: {}. Moving image to folder: {}",
                    letter,
                    &dest_dir
                );
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
                                self.image_manager.set_image_folder(folder);
                            }
                        }
                    }
                    ui.label("Selected Folder:");
                    match &self.selected_folder {
                        Some(folder) => ui.label(folder),
                        None => ui.label("No folder selected."),
                    };
                    ui.label(format!("({})", self.image_manager.num_images()));
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

                // TODO: Tidy this up. It used to be in if let below but was
                // extracted due to borrow checker.
                let n_out_of_all = format!(
                    "({}/{})",
                    self.image_manager.current_index() + 1,
                    self.image_manager.num_images(),
                );
                // Display the current image:
                if let Some(image_info) = self.image_manager.load_current_image() {
                    let filename = get_file_name(&image_info.path);
                    ui.label(format!("Current Image: {} {}", n_out_of_all, filename));
                    ui.add(image_info.image.fit_to_exact_size(image_area.size()));
                } else if !self.image_manager.num_images() == 0 {
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
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(egui::Vec2::new(1280.0, 960.0)),
        ..Default::default()
    };

    let noto_sans_font = FontAsset::get("NotoSansJP-VariableFont_wght.ttf").unwrap();

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        FontData::from_owned(noto_sans_font.data.to_vec()).into(),
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
        let src_path = temp_dir.path().join("test.jpg");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path, b"Hello, world!").unwrap();
        assert!(src_path.exists());
        move_file(&src_path.to_string_lossy(), &dest_dir.to_string_lossy()).unwrap();
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.jpg").exists());
    }

    #[test]
    fn move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("test.jpg");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path, b"Hello, world!").unwrap();
        app.image_manager
            .set_image_folder(&temp_dir.path().to_string_lossy());
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.jpg").exists());
    }

    // Given there are mulitple files in the src folder, move the current image to the dest folder.
    #[test]
    fn multiple_one_file_move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path1 = temp_dir.path().join("test1.jpg");
        let src_path2 = temp_dir.path().join("test2.jpg");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path1, b"Hello, world!").unwrap();
        std::fs::write(&src_path2, b"Hello, world!").unwrap();
        app.image_manager
            .set_image_folder(&temp_dir.path().to_string_lossy());

        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();

        // Which file gets moved as a result of move_current_image_to_dest() changes depending on
        // implementation.
        assert!(src_path1.exists());
        assert!(!src_path2.exists());
        assert!(!dest_dir.join("test1.jpg").exists());
        assert!(dest_dir.join("test2.jpg").exists());
    }

    #[test]
    fn move_all_files_move_current_image_to_dest_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path1 = temp_dir.path().join("test1.jpg");
        let src_path2 = temp_dir.path().join("test2.jpg");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();
        std::fs::write(&src_path1, b"Hello, world!").unwrap();
        std::fs::write(&src_path2, b"Hello, world!").unwrap();
        app.image_manager
            .set_image_folder(&temp_dir.path().to_string_lossy());
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();
        assert!(!src_path1.exists());
        assert!(!src_path2.exists());
        assert!(dest_dir.join("test1.jpg").exists());
        assert!(dest_dir.join("test2.jpg").exists());
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
        let src_path = temp_dir.path().join("test.jpg");
        let dest_dir = temp_dir.path().join("test_dest");
        fs::create_dir(&dest_dir).unwrap();

        std::fs::write(&src_path, b"Hello, world!").unwrap();
        app.image_manager
            .set_image_folder(&temp_dir.path().to_string_lossy());

        app.move_current_image_to_dest(&dest_dir.to_string_lossy())
            .unwrap();

        // Make sure its not in image paths anymore and has been moved.
        assert!(!src_path.exists());
        assert!(dest_dir.join("test.jpg").exists());

        // Now undo and check that everything is rolled back.
        let Some(undo_path) = app.undo_move() else {
            panic!("undo_move() returned None");
        };
        assert_eq!(undo_path, src_path.to_string_lossy());
        assert!(src_path.exists());
        assert!(!dest_dir.join("test.jpg").exists());

        // Further undo should return None.
        assert!(app.undo_move().is_none());
        assert!(app.undo_move().is_none());
    }

    #[test]
    fn remove_current_image_test() {
        let mut app = MyApp::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let src_path = temp_dir.path().join("test.jpg");
        std::fs::write(&src_path, b"Hello, world!").unwrap();
        app.image_manager
            .set_image_folder(&temp_dir.path().to_string_lossy());
        assert!(app.image_manager.load_current_image().is_some());
        let Some(path) = app.image_manager.remove_current_image() else {
            panic!("remove_current_image() returned None");
        };
        assert_eq!(path, src_path.to_string_lossy());
        assert!(app.image_manager.load_current_image().is_none());
    }
}
