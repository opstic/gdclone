use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use bevy::{
    asset::{AssetIo, AssetIoError, Metadata},
    prelude::*,
    utils::BoxedFuture,
};
use bevy::asset::FileAssetIo;
use bevy::utils::HashMap;
use directories::{BaseDirs, ProjectDirs};
use futures_lite::future;
use native_dialog::{FileDialog, MessageDialog, MessageType};
use serde::{Deserialize, Serialize};
use steamlocate::SteamDir;

struct MultiAssetIo {
    prefixes: HashMap<String, Box<dyn AssetIo>>,
}

fn path_separator(char: char) -> bool {
    char == '/' || char == '\\'
}

impl AssetIo for MultiAssetIo {
    fn load_path<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<Vec<u8>, AssetIoError>> {
        let path_str = path.to_str().unwrap();
        let split_path: Vec<&str> = path_str.splitn(2, path_separator).collect();
        if !split_path[0].starts_with(':') {
            if let Some(asset_io) = self.prefixes.get("default") {
                asset_io.load_path(path)
            } else {
                Box::pin(future::ready(Err(AssetIoError::NotFound(
                    path.to_path_buf(),
                ))))
            }
        } else if let Some(asset_io) = self.prefixes.get(split_path[0].trim_start_matches(':')) {
            asset_io.load_path(Path::new(if split_path.len() < 2 {
                "."
            } else {
                split_path[1].trim_start_matches(path_separator)
            }))
        } else {
            Box::pin(future::ready(Err(AssetIoError::NotFound(
                path.to_path_buf(),
            ))))
        }
    }

    fn read_directory(
        &self,
        path: &Path,
    ) -> Result<Box<dyn Iterator<Item=PathBuf>>, AssetIoError> {
        let path_str = path.to_str().unwrap();
        let split_path: Vec<&str> = path_str.splitn(2, path_separator).collect();
        if !split_path[0].starts_with(':') {
            if let Some(asset_io) = self.prefixes.get("default") {
                asset_io.read_directory(path)
            } else {
                Err(AssetIoError::NotFound(path.to_path_buf()))
            }
        } else if let Some(asset_io) = self.prefixes.get(split_path[0].trim_start_matches(':')) {
            asset_io.read_directory(Path::new(if split_path.len() < 2 {
                "."
            } else {
                split_path[1].trim_start_matches(path_separator)
            }))
        } else {
            Err(AssetIoError::NotFound(path.to_path_buf()))
        }
    }

    fn get_metadata(&self, path: &Path) -> Result<Metadata, AssetIoError> {
        let path_str = path.to_str().unwrap();
        let split_path: Vec<&str> = path_str.splitn(2, path_separator).collect();
        if !split_path[0].starts_with(':') {
            if let Some(asset_io) = self.prefixes.get("default") {
                asset_io.get_metadata(path)
            } else {
                Err(AssetIoError::NotFound(path.to_path_buf()))
            }
        } else if let Some(asset_io) = self.prefixes.get(split_path[0].trim_start_matches(':')) {
            asset_io.get_metadata(Path::new(if split_path.len() < 2 {
                "."
            } else {
                split_path[1].trim_start_matches(path_separator)
            }))
        } else {
            Err(AssetIoError::NotFound(path.to_path_buf()))
        }
    }

    fn watch_path_for_changes(
        &self,
        to_watch: &Path,
        to_reload: Option<PathBuf>,
    ) -> Result<(), AssetIoError> {
        let watch_path_str = to_watch.to_str().unwrap();
        let watch_split_path: Vec<&str> = watch_path_str.splitn(2, path_separator).collect();
        let reload_path = match to_reload.clone() {
            Some(reload_path) => {
                let reload_path_str = reload_path.to_str().unwrap();
                let reload_split_path: Vec<&str> =
                    reload_path_str.splitn(2, path_separator).collect();
                Some(PathBuf::from(reload_split_path.get(1).unwrap_or(&".")))
            }
            None => None,
        };
        if !watch_split_path[0].starts_with(':') {
            if let Some(asset_io) = self.prefixes.get("default") {
                asset_io.watch_path_for_changes(to_watch, to_reload)
            } else {
                Err(AssetIoError::NotFound(to_watch.to_path_buf()))
            }
        } else if let Some(asset_io) = self
            .prefixes
            .get(watch_split_path[0].trim_start_matches(':'))
        {
            asset_io.watch_path_for_changes(
                Path::new(if watch_split_path.len() < 2 {
                    "."
                } else {
                    watch_split_path[1].trim_start_matches(path_separator)
                }),
                reload_path,
            )
        } else {
            Err(AssetIoError::NotFound(to_watch.to_path_buf()))
        }
    }

    fn watch_for_changes(&self) -> Result<(), AssetIoError> {
        for (_, asset_io) in &self.prefixes {
            asset_io.watch_for_changes()?
        }
        Ok(())
    }
}

pub(crate) struct MultiAssetIoPlugin;

const GEOMETRY_DASH_APP_ID: u32 = 322170;

#[derive(Serialize, Deserialize, Debug, Default)]
struct PathConfig {
    gd_path: String,
    gd_data_path: String,
}

impl Plugin for MultiAssetIoPlugin {
    fn build(&self, app: &mut App) {
        let default_asset_plugin = AssetPlugin::default();

        let mut prefixes: HashMap<String, Box<dyn AssetIo>> = HashMap::new();

        let project_dirs = ProjectDirs::from("dev", "Opstic", "GDClone").unwrap();
        let base_dirs = BaseDirs::new().unwrap();

        std::fs::create_dir_all(project_dirs.config_local_dir()).unwrap();

        let config_path = project_dirs.config_local_dir().join("path_config.json");

        let mut path_config = if let Ok(config_file) = File::open(config_path.clone()) {
            serde_json::from_reader(BufReader::new(config_file)).unwrap_or_default()
        } else {
            PathConfig::default()
        };

        prefixes.insert(
            "default".to_string(),
            default_asset_plugin.create_platform_default_asset_io(),
        );

        let mut gd_prefixes: HashMap<String, Box<dyn AssetIo>> = HashMap::new();

        let config_gd_path = PathBuf::from(&path_config.gd_path);

        let gd_path = if config_gd_path.join("Resources").is_dir() {
            config_gd_path
        } else if let Some(path) = match SteamDir::locate() {
            Some(mut steam_dir) => steam_dir
                .app(&GEOMETRY_DASH_APP_ID)
                .map(|app| app.path.clone()),
            None => None,
        } {
            path
        } else {
            MessageDialog::new()
                .set_type(MessageType::Error)
                .set_title("Error when locating")
                .set_text(
                    "Cannot locate Geometry Dash. Please select the install directory manually.",
                )
                .show_alert()
                .unwrap();

            let mut gd_path = PathBuf::new();

            while !gd_path.join("Resources").is_dir() {
                let selected_path = FileDialog::new().show_open_single_dir().unwrap();
                if let Some(selected_path) = selected_path {
                    let resources_path = selected_path.join("Resources");
                    if resources_path.is_dir() {
                        gd_path = selected_path;
                    } else {
                        MessageDialog::new()
                            .set_type(MessageType::Error)
                            .set_title("Error when locating")
                            .set_text(
                                "This directory does not contain the necessary files. Please select the correct directory.",
                            )
                            .show_alert()
                            .unwrap();
                    }
                } else {
                    std::process::exit(0);
                }
            }
            gd_path
        };

        gd_prefixes.insert(
            "resources".to_string(),
            Box::new(FileAssetIo::new(
                gd_path.join("Resources"),
                default_asset_plugin.watch_for_changes,
            )),
        );

        path_config.gd_path = gd_path.into_os_string().into_string().unwrap();

        let config_gd_data_path = PathBuf::from(path_config.gd_data_path);

        let gd_data_path = if config_gd_data_path.join("CCLocalLevels.dat").is_file() {
            config_gd_data_path
        } else if base_dirs
            .data_local_dir()
            .join("GeometryDash/CCLocalLevels.dat")
            .is_file()
        {
            base_dirs.data_local_dir().join("GeometryDash")
        } else {
            MessageDialog::new()
                .set_type(MessageType::Error)
                .set_title("Error when locating")
                .set_text(
                    "Cannot locate Geometry Dash's data directory. Please select the data directory manually.",
                )
                .show_alert()
                .unwrap();

            let mut gd_data_path = PathBuf::new();

            while !gd_data_path.join("CCLocalLevels.dat").is_file() {
                let selected_path = FileDialog::new().show_open_single_dir().unwrap();
                if let Some(selected_path) = selected_path {
                    let levels_path = selected_path.join("CCLocalLevels.dat");
                    if levels_path.is_file() {
                        gd_data_path = selected_path;
                    } else {
                        MessageDialog::new()
                            .set_type(MessageType::Error)
                            .set_title("Error when locating")
                            .set_text(
                                "This directory does not contain the necessary files. Please select the correct directory.",
                            )
                            .show_alert()
                            .unwrap();
                    }
                } else {
                    MessageDialog::new()
                        .set_type(MessageType::Warning)
                        .set_title("Select a directory")
                        .set_text("Please select a directory.")
                        .show_alert()
                        .unwrap();
                }
            }
            gd_data_path
        };

        gd_prefixes.insert(
            "data".to_string(),
            Box::new(FileAssetIo::new(
                gd_data_path.clone(),
                default_asset_plugin.watch_for_changes,
            )),
        );

        path_config.gd_data_path = gd_data_path.into_os_string().into_string().unwrap();

        let mut config_file = File::create(config_path).unwrap();

        config_file
            .write_all(
                serde_json::to_string_pretty(&path_config)
                    .unwrap()
                    .as_bytes(),
            )
            .unwrap();

        let gd_multi_asset_io = MultiAssetIo {
            prefixes: gd_prefixes,
        };

        prefixes.insert("gd".to_string(), Box::new(gd_multi_asset_io));

        // create the custom asset io instance
        let asset_io = MultiAssetIo { prefixes };

        // the asset server is constructed and added the resource manager
        app.insert_resource(AssetServer::new(asset_io));
    }
}
