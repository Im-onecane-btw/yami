// Действия панелей шелла. Сам немедленный UI (кнопки, поля, зоны) — общий, в yami-px:
// им же рисуют файловый менеджер и диалоги.

pub use yami_px::ui::{Cur, Field, Tone};

pub type Ui<'a, 'b> = yami_px::ui::Ui<'a, 'b, Act>;
pub type Zone = yami_px::ui::Zone<Act>;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// глушит клики (фон панели)
    Block,
    CloseModal,
    // бар
    Launcher,
    Power,
    Settings,
    Ws(u64),
    Task(u64),
    Tray(String),
    Mode,
    Kb,
    Popup(&'static str),
    // лаунчер
    LRun(usize),
    LTab(bool),
    LPin(usize),
    LDel(usize),
    LWipe,
    // питание
    PowerDo(usize),
    // настройки
    STab(&'static str),
    SPreset(usize),
    SBool(&'static str),
    SSlider(&'static str, f32, f32, i32),
    SCycle(&'static str),
    /// число в пикселях (config::NUMS): на шаг вниз (-1) или вверх (+1); колесо — тоже
    SNum(&'static str, i32),
    /// вкладка «обновления»: выбрать, куда откатиться («prev» — прошлая сборка, иначе версия)
    UpdSel(String),
    /// шторка для трансляций: опустить / поднять
    Curtain,
    /// вкладка «обои» (wall.rs)
    SWall(&'static str),
    /// выбор обоев (walls.rs): клетка, колесо над сеткой, другая папка, другой файл, назад
    WallPick(usize),
    WallsScroll,
    WallsDir,
    WallsFile,
    WallsBack,
    /// модуль бара во вкладке «бар»: тащить (сторона, индекс), новый, убрать
    SModDrag(usize, usize),
    SModNew(String),
    /// щелчок по модулю-скрипту: его команда
    Plug(String),
    NightToggle,
    ReplySend,
    SModDel(usize, usize),
    SWidgetAdd(&'static str),
    SWidgetDel(u32),
    SWidgetOpt(u32, &'static str),
    SWidgetSel(u32),
    SEdit,
    SResetPos,
    SField(&'static str),
    // меню
    Menu(usize),
    MenuBack,
    // всплывашки
    CalMonth(i32),
    VolSlider,
    VolMute,
    Sink(String),
    /// микрофон по умолчанию (всплывашка звука, #10)
    Source(String),
    MicSlider,
    MicMute,
    AppVol(u32),
    AppMute(u32),
    MediaPlay,
    MediaNext,
    MediaPrev,
    /// перемотка по полосе позиции (тянуть; сама перемотка — при отпускании)
    MediaSeek,
    Wifi(bool),
    WifiNet(String),
    WifiRescan,
    NetEditor,
    BtPower(bool),
    BtDev(String),
    BtScan,
    /// полный менеджер bluetooth (blueman или bluetoothctl в терминале)
    BtManager,
    Dnd,
    NotifClear,
    NotifDel(u32),
    NotifAct(u32, String),
    NotifClick(u32),
    Pavu,
    SysMon,
    // заголовки окон
    TDrag(u64),
    TMin(u64),
    TMax(u64),
    TClose(u64),
    // виджеты
    WDrag(u32),
    WMenu(u32),
    EditDone,
    SnapBtn(&'static str),
    DockItem(usize),
    DockLauncher,
    DockTrash,
    DockPin(String, i32),
    LockField,
    AuthField,
    AuthOk,
    AuthCancel,
    Prof(String),
    ScreenSel(usize),
    ScreenOk,
    ScreenCancel,
    Disp(&'static str),
    DispSel(usize),
    DispDrag(usize),
    // клавиши (keys.rs)
    KeyCat(&'static str),
    KeyRec(String),
    KeyReset(String),
    KeyDel(usize),
    KeyAdd,
    KeyScroll,
}

impl Act {
    pub fn is_drag(&self) -> bool {
        matches!(self, Act::SSlider(..) | Act::VolSlider | Act::MicSlider | Act::AppVol(_) | Act::MediaSeek | Act::TDrag(_) | Act::WDrag(_) | Act::DispDrag(_) | Act::SModDrag(..) | Act::SModNew(_))
    }
}

impl yami_px::ui::Action for Act {
    fn block() -> Self {
        Act::Block
    }
}
