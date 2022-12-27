use std::{
    collections::HashMap,
    fs::{self, read_dir, File},
    io::Write,
    time::{Duration, SystemTime},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const BASE_PATH: &str = "/run/user/1000/gvfs";
pub const SONGS_PATH: &str =
    "/Internal shared storage/ModData/com.beatgames.beatsaber/Mods/SongLoader/CustomLevels";
pub const PLAYLISTS_PATH: &str =
    "/Internal shared storage/ModData/com.beatgames.beatsaber/Mods/PlaylistManager/Playlists";

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct CustomLevel {
    #[serde(alias = "_version")]
    pub version: String,
    #[serde(alias = "_songName")]
    pub song_name: String,
    #[serde(alias = "_songSubName")]
    pub song_sub_name: String,
    #[serde(alias = "_songAuthorName")]
    pub song_author: String,
    #[serde(alias = "_levelAuthorName")]
    pub level_author: String,
    #[serde(alias = "_coverImageFilename")]
    pub cover_image_filename: String,
    #[serde(alias = "_beatsPerMinute")]
    pub beats_per_minute: f32,
    pub hash: Option<String>,
    pub modified: Option<u128>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Playlist {
    #[serde(rename = "playlistTitle")]
    pub title: String,
    #[serde(rename = "playlistDescription")]
    pub description: Option<String>,
    pub songs: Vec<Song>,
    #[serde(skip)]
    pub changed: bool,
    #[serde(skip)]
    pub just_created: bool,
    #[serde(skip)]
    pub file_name: String,
    #[serde(rename = "imageString")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "playlistAuthor")]
    pub author: Option<String>,
    #[serde(rename = "syncURL")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_url: Option<String>,
    #[serde(rename = "customData")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Song {
    pub hash: String,
    #[serde(alias = "songName")]
    pub name: String,
}

pub fn get_device_folder() -> String {
    let device_folder = match fs::read_dir(BASE_PATH) {
        Ok(read_dir) => {
            let oculus_dir = read_dir.flatten().find(|dir_entry| {
                return dir_entry
                    .file_name()
                    .to_str()
                    .unwrap_or("none")
                    .to_lowercase()
                    .contains("quest");
            });

            let oculus_dir = match oculus_dir {
                Some(dir) => dir.file_name().to_str().unwrap_or("none").to_owned(),
                None => "none".to_string(),
            };

            oculus_dir
        }
        Err(_) => "none".to_string(),
    };

    device_folder
}

pub fn get_custom_levels() -> Vec<CustomLevel> {
    let oculus_folder = get_device_folder();

    match read_from_cache::<CustomLevel>("custom_levels.json") {
        Some(cached_levels) => match count_custom_levels_on_device(&oculus_folder) {
            Some(number_of_levels_on_device) => {
                if number_of_levels_on_device != cached_levels.len() {
                    println!(
                        "There are {} songs on device, but {} cached, invalidating cahce.",
                        number_of_levels_on_device,
                        cached_levels.len()
                    );
                    let levels_from_device = read_custom_levels_from_device(&oculus_folder);
                    cache(&levels_from_device, "custom_levels.json");
                    levels_from_device
                } else {
                    cached_levels
                }
            }
            None => cached_levels,
        },
        None => {
            let levels_from_device = read_custom_levels_from_device(&oculus_folder);
            cache(&levels_from_device, "custom_levels.json");
            levels_from_device
        }
    }
}

pub fn get_playlists() -> Vec<Playlist> {
    read_playlists_from_device(&get_device_folder())
}

pub fn save_modified_playlists(playlists: &[Playlist]) {
    let count = playlists.iter().filter(|p| p.changed).count();
    println!("Trying to save {} playlists.", count);
    //let path = format!("{}/{}{}", BASE_PATH, get_device_folder(), PLAYLISTS_PATH);
    let path = "";
    playlists
        .iter()
        .filter(|playlist| playlist.changed)
        .map(|playlist| {
            (
                serde_json::to_string(playlist).unwrap_or_else(|_| " ".to_string()),
                &playlist.file_name,
            )
        })
        .for_each(|(serialized_playlist, file_name)| {
            // match File::create(format!("{}/{}", path, file_name)) {
            match File::create(file_name) {
                Ok(mut file) => match file.write_all(serialized_playlist.as_bytes()) {
                    Ok(_) => println!("Playlist saved to {}", file_name),
                    Err(_) => println!("Can't save playlist to {}", file_name),
                },
                Err(_) => println!("Can't create playlist file."),
            }
        });
}

pub fn is_playlist_contains_song(playlist: &Playlist, song: Song) -> bool {
    playlist
        .songs
        .iter()
        .any(|playlist_song| playlist_song.hash == song.hash)
}

fn read_playlists_from_device(oculus_folder_name: &str) -> Vec<Playlist> {
    let path = format!("{}/{}{}", BASE_PATH, oculus_folder_name, PLAYLISTS_PATH);
    let mut playlists = Vec::<Playlist>::new();

    match fs::read_dir(&path) {
        Ok(read_dir) => {
            for dir_entry in read_dir.flatten() {
                if let Ok(string) = fs::read_to_string(dir_entry.path()) {
                    if let Ok(mut playlist) = serde_json::from_str::<Playlist>(&string) {
                        playlist.changed = false;
                        playlist.file_name = dir_entry
                            .file_name()
                            .to_str()
                            .unwrap_or("default_playlist_name.json")
                            .to_string();
                        playlists.push(playlist);
                    } else {
                        println!(
                            "Can't deserialize: {}.",
                            dir_entry.file_name().to_str().unwrap_or("Unknown")
                        );
                    }
                }
            }
        }
        Err(_) => println!("Can't access {}", &path),
    }

    playlists
}

fn read_custom_levels_from_device(oculus_folder_name: &str) -> Vec<CustomLevel> {
    let mut custom_levels = Vec::<CustomLevel>::new();
    let path = format!("{}/{}{}", BASE_PATH, oculus_folder_name, SONGS_PATH);

    match fs::read_dir(format!(
        "{}/{}{}",
        BASE_PATH, oculus_folder_name, SONGS_PATH
    )) {
        Ok(read_dir) => read_dir.flatten().for_each(|dir_entry| {
            let hash = dir_entry
                .file_name()
                .to_str()
                .unwrap_or("missing_hash")
                .to_owned();

            let modified = match dir_entry.metadata() {
                Ok(metadata) => match metadata.modified() {
                    Ok(time) => time,
                    Err(_) => SystemTime::now(),
                },
                Err(_) => SystemTime::now(),
            };

            let modified = modified.elapsed().unwrap_or(Duration::ZERO).as_millis();

            match fs::read_to_string(format!("{}/{}/Info.dat", path, hash))
                .or_else(|_| fs::read_to_string(format!("{}/{}/info.dat", path, hash)))
            {
                Ok(info_dat) => {
                    println!("Reading level: {}, number: {}", hash, custom_levels.len());
                    match serde_json::from_str::<CustomLevel>(&info_dat) {
                        Ok(mut level) => {
                            level.hash = Some(hash);
                            level.modified = Some(modified);
                            custom_levels.push(level);
                        }
                        Err(_) => println!("Can't deserialize info.dat in the folder: {}", hash),
                    }
                }
                Err(err) => {
                    println!("Can't read info.dat from folder with name: {}", hash);
                    match err.kind() {
                        std::io::ErrorKind::NotFound => println!("File not found."),
                        std::io::ErrorKind::PermissionDenied => println!("Permission denied."),
                        std::io::ErrorKind::Interrupted => println!("Interrupted."),
                        std::io::ErrorKind::InvalidInput => println!("Invalid input."),
                        std::io::ErrorKind::AlreadyExists => println!("Already exists."),
                        _ => println!("Unknown error."),
                    }
                }
            }
        }),
        Err(_) => println!(
            "Can't open CustomLevels folder on the {} device.",
            oculus_folder_name
        ),
    }
    custom_levels
}

fn cache<T: Serialize>(entities: &[T], file_name: &str) {
    let serialized = serde_json::to_string(entities).unwrap_or_else(|_| "[]".to_string());
    println!("Attempting to serialize entities to {}", file_name);

    match File::create(file_name) {
        Ok(mut file) => match file.write_all(serialized.as_bytes()) {
            Ok(_) => println!("Entities cached to {}", file_name),
            Err(_) => println!("Can't write entities to {}", file_name),
        },
        Err(_) => println!("Can't create cache file."),
    }
}

fn read_from_cache<T: DeserializeOwned>(file_name: &str) -> Option<Vec<T>> {
    match fs::read_to_string(file_name) {
        Ok(string) => {
            let entities =
                serde_json::from_str::<Vec<T>>(string.as_str()).unwrap_or_else(|_| Vec::<T>::new());
            println!("Entities retrived from cache.");
            Some(entities)
        }
        Err(_) => {
            println!("Can't read cached entities.");
            None
        }
    }
}

fn count_custom_levels_on_device(oculus_folder_name: &str) -> Option<usize> {
    let path = format!("{}/{}{}", BASE_PATH, oculus_folder_name, SONGS_PATH);
    match fs::read_dir(path) {
        Ok(dir) => Some(dir.count()),
        Err(_) => None,
    }
}
