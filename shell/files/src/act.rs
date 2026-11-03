// Всё, по чему можно щёлкнуть в окне файлов.

use crate::fm::Mode;
use crate::fsx::Sort;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Block,
    Back,
    Fwd,
    Up,
    PathBar,
    Crumb(PathBuf),
    PathField,
    Place(PathBuf, bool),
    PlaceTrash,
    Dev(usize),
    Eject(usize),
    Item(usize, usize),
    Bg(usize),
    Bar(usize),
    ColSort(Sort),
    Tab(usize),
    TabClose(usize),
    TabNew,
    ModeBtn(Mode),
    SplitBtn,
    InfoBtn,
    MenuBtn,
    SearchBtn,
    FilterField,
    FilterClear,
    MItem(usize),
    MSub(usize),
    MenuBg,
    Dlg(&'static str),
    DlgField,
    DlgApp(usize),
    DlgBit(u32),
    SBool(&'static str),
    SCycle(&'static str),
    SZoom,
    PickOk,
    PickCancel,
    PickName,
    PickFilter,
    JobCancel(u32),
    Toast,
    Drag,
    Dim,
}

impl yami_px::ui::Action for Act {
    fn block() -> Self {
        Act::Block
    }
}

pub type Ui<'a, 'b> = yami_px::ui::Ui<'a, 'b, Act>;
