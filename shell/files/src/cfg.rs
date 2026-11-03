// Настройки файлового менеджера: ~/.config/yami-files.json (палитра и язык — из шелла).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Cfg {
    /// grid | list
    pub view: String,
    /// размер значков в сетке: 0…4
    pub zoom: i32,
    pub hidden: bool,
    pub folders_first: bool,
    /// name | size | time | type
    pub sort: String,
    pub desc: bool,
    pub thumbs: bool,
    /// миниатюры для картинок не больше, МБ
    pub thumb_max_mb: u32,
    pub sidebar: bool,
    /// диски, флешки, телефоны и камеры в боковой панели
    pub devices: bool,
    /// раздел «сеть»: сетевые ресурсы и «подключиться к серверу»
    pub network: bool,
    /// адреса серверов, к которым подключались (без паролей)
    pub servers: Vec<String>,
    /// панель сведений справа
    pub info: bool,
    pub terminal: String,
    pub confirm_trash: bool,
    pub single_click: bool,
    /// открывать новое окно в разделённом виде
    pub split: bool,
    pub width: i32,
    pub height: i32,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            view: "grid".into(),
            zoom: 2,
            hidden: false,
            folders_first: true,
            sort: "name".into(),
            desc: false,
            thumbs: true,
            thumb_max_mb: 40,
            sidebar: true,
            devices: true,
            network: true,
            servers: vec![],
            info: false,
            terminal: "kitty".into(),
            confirm_trash: false,
            single_click: false,
            split: false,
            width: 1060,
            height: 660,
        }
    }
}

pub fn path() -> std::path::PathBuf {
    crate::paths::config_dir().join("yami-files.json")
}

impl Cfg {
    pub fn load() -> Cfg {
        std::fs::read_to_string(path()).map(|t| crate::paths::lenient_json(&t)).unwrap_or_default()
    }
    pub fn save(&self) {
        let _ = crate::paths::write_atomic(&path(), &(serde_json::to_string_pretty(self).unwrap_or_default() + "\n"));
    }
    /// клетка значка в сетке, px: 16·u
    pub fn icon_u(&self) -> i32 {
        [2, 3, 4, 5, 6][self.zoom.clamp(0, 4) as usize]
    }
}
