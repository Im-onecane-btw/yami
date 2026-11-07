# Сборка пакетов

[English](PACKAGING.md) · Русский

Заметки для пользователей Nix и сборщиков пакетов дистрибутивов: flake, где yami ищет системные файлы и как отдать обновления пакетному менеджеру.

## Nix

Во flake есть пакет (шелл, yami-files, yami-view, yami-play и экран входа yami-greet), модуль NixOS и модуль Home Manager. Модуль NixOS — системная половина `install.sh`: niri, службы, с которыми говорит шелл, порталы с диалогом файлов и выбором показа экрана от yami, шрифты, команды, которые запускает шелл, и экран входа. Модуль Home Manager — домашняя половина: конфиг niri, настройки шелла, kitty, fontconfig и программы по умолчанию.

```nix
{
  inputs.yami.url = "github:Trendorin/yami";
  inputs.yami.inputs.nixpkgs.follows = "nixpkgs";

  # NixOS
  imports = [ yami.nixosModules.default ];
  programs.yami.enable = true;
  programs.yami.loginScreen.user = "alice"; # чей шелл задаёт вид экрана входа

  # Home Manager
  imports = [ yami.homeManagerModules.default ];
  programs.yami.enable = true;
  programs.yami.settings = { palette = "sakura"; lang = "en"; };
}
```

Параметры NixOS:

| Параметр | По умолчанию | Что |
|---|---|---|
| `programs.yami.enable` | `false` | niri, NetworkManager, BlueZ, udisks, gvfs, polkit, профили питания, связка ключей, порталы, шрифты и команды шелла |
| `programs.yami.loginScreen.enable` | `true` | greetd запускает yami-greet внутри niri; выключенный оставляет другой менеджер входа |
| `programs.yami.loginScreen.user` | `null` | пользователь, чей шелл пишет вид экрана входа (палитра, обои, язык, экраны, раскладки) |
| `programs.yami.apps` | `true` | kitty, fastfetch, pavucontrol, btop, миниатюры видео, bsdtar |
| `programs.yami.tools` | `true` | ночной свет, текст и QR-коды со скриншотов |
| `programs.yami.appTheme` | `true` | qt6ct, adw-gtk3 и Papirus для программ Qt и GTK в палитре |

Параметры Home Manager: `settings` (ключи `yami-shell.json`, сливаются при каждой активации: эти главнее, остальное из панели настроек остаётся), `niriConfig` (`config.kdl` от yami, копируется, пока `~/.config/niri/.yami` помечает его как файл yami; свои правки — в `user.kdl`), `kitty`, `defaultApplications`, `extraPackages`.

Шелл сам пишет свои файлы во время работы (настройки, клавиши, экраны, порталы), поэтому модули кладут их один раз и оставляют доступными для записи. yami-niri, исправленный niri из `fork/`, пакетом не собирается: пакетный niri — тот же стол без разрывов в играх, главного потока реального времени и правил слоёв для шторки трансляций. `nix flake check` собирает пакет и вычисляет модуль NixOS; CI запускает его при каждом пуше. Модули начинались с работы TheCodedKid в #1.

## Системные файлы

Шелл находит помощника polkit и бэкенд MTP из gvfs без жёстких путей FHS. На NixOS помощник polkit — setuid-обёртка `/run/wrappers/bin/polkit-agent-helper-1`, она идёт первой: копия помощника из `PATH` без бита setuid пароль не проверит. gvfs находится через `XDG_DATA_DIRS`. Старые пути в `/usr` остаются запасными.

Экран входа: greetd запускает `libexec/yami/greet-session` от своего пользователя greeter (установщик кладёт его в `/usr/local/lib/yami`). Он находит `bin/yami-greet` от своей папки (`YAMI_GREET` задаёт другой), niri — в `PATH`, сеансы — в папках `wayland-sessions` из `XDG_DATA_DIRS`, а yami-greet находит свои шрифты в `share/yami-greet/fonts` рядом со своим бинарником (`YAMI_GREET_FONTS`). Нужны три папки: `/var/lib/yami-greet/look` пользователя, чей шелл выкладывает вид, `/var/lib/yami-greet/state` (0700) и `/var/lib/yami-greet/log` пользователя greeter.

## Обновления через пакетный менеджер

Соберите с признаком Cargo `package-managed` (flake так и делает):

```sh
cargo build --release --locked --workspace --features yami-shell/package-managed --manifest-path shell/Cargo.toml
```

Такая сборка не проверяет обновления по расписанию, вкладка обновлений показывает заметку вместо кнопок, а `yami update` (с любой подкомандой) пишет «Обновлениями управляет пакетный менеджер.» и выходит с 1. Обновление, переход на выпуск и откат отказываются раньше, чем тронут клон исходников или установленные бинари. Обновляйте и откатывайте пакетным менеджером. Сборка без признака сохраняет обновления.

Не запускайте `install.sh` поверх установки, которой владеет пакетный менеджер.
