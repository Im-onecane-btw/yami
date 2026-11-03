// YAMI // шелл для niri на Rust: бар, лаунчер, буфер, питание, настройки, уведомления,
// трей, всплывашки (календарь, звук, сеть, bluetooth, система), виджеты рабочего стола,
// заголовки окон в режиме «стол», блокировка. Рисует сам, пикселями, в shm — без тулкитов.
//
//   yami-shell                запустить
//   yami-shell msg <команда>  launcher | clipboard | power | lock | bar | stacking |
//                                 minimize | restore | settings | edit | reload |
//                                 screenshot [area|window|screen] | wallpaper [путь] | curtain [on|off]
//   yami-shell shot [цель…]       снимки панелей в PNG без Wayland (см. shot.rs)

pub use yami_px::{font, gfx, icons, theme};

mod app;
mod auth;
mod profile;
mod displays;
mod screen;
mod audio;
mod bar;
mod cli;
mod click;
mod dbus;
mod dock;
mod fonttab;
mod games;
mod i18n;
mod config;
mod curtain;
mod ipc;
mod keys;
mod launcher;
mod lock;
mod media;
mod menu;
mod net;
mod niri;
mod notif;
mod pam;
mod popups;
mod power;
mod settings;
mod shot;
mod snap;
mod sys;
mod titles;
mod ui;
mod wall;
mod widgets;

fn main() {
    // большие буферы (обои, скриншоты, обложки) — сразу через mmap и обратно системе при
    // освобождении: иначе glibc поднимает порог и держит их в куче нитей после каждого декода
    unsafe {
        libc::mallopt(libc::M_MMAP_THRESHOLD, 256 << 10);
        libc::mallopt(libc::M_ARENA_MAX, 2);
    }
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("msg") => {
            let cmd = args[2..].join(" ");
            std::process::exit(if ipc::send(&cmd) { 0 } else { 1 });
        }
        Some("-h") | Some("--help") => {
            println!("yami-shell                 run the shell (niri starts it)\nyami-shell msg <launcher|clipboard|power|lock|bar|stacking|minimize|restore|settings|edit|reload|profile|popup KIND|screenshot [area|window|screen]|wallpaper [PATH]|curtain [on|off]|quit>\nyami-shell config …        read and change settings from the terminal (yami-shell config help)\nyami-shell binds …         keybindings: list, print, write\nyami-shell shot [TARGET…] [--lang en] [--defaults] [-o DIR]   draw panels offscreen to PNG");
        }
        Some("--version") => println!("yami-shell {}", env!("CARGO_PKG_VERSION")),
        Some("shot") => std::process::exit(shot::run(&args[2..])),
        Some("config") | Some("binds") => {
            // `… | head` не должен падать с паникой
            unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
            std::process::exit(if args[1] == "config" { cli::config(&args[2..]) } else { cli::binds(&args[2..]) })
        }
        _ => {
            config::migrate();
            app::run()
        }
    }
}
