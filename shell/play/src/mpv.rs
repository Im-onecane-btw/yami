// libmpv, загруженная при запуске (dlopen): весь разбор форматов, декодирование (и аппаратное),
// звук, субтитры и синхронизация — у mpv; от нас только окно и пиксельный интерфейс.
// Кадры mpv рисует программно (render API «sw») прямо в наш shm-буфер, без OpenGL.
// Сборке libmpv не нужна: нет библиотеки — плеер говорит, какой пакет поставить.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

const FMT_STRING: c_int = 1;
const FMT_FLAG: c_int = 3;
const FMT_INT64: c_int = 4;
const FMT_DOUBLE: c_int = 5;

pub const EV_NONE: c_int = 0;
pub const EV_SHUTDOWN: c_int = 1;
pub const EV_END_FILE: c_int = 7;
pub const EV_FILE_LOADED: c_int = 8;
pub const EV_VIDEO_RECONFIG: c_int = 17;
pub const EV_PROPERTY_CHANGE: c_int = 22;

// render.h
const RP_INVALID: c_int = 0;
const RP_API_TYPE: c_int = 1;
const RP_BLOCK_FOR_TARGET_TIME: c_int = 12;
const RP_SW_SIZE: c_int = 17;
const RP_SW_FORMAT: c_int = 18;
const RP_SW_STRIDE: c_int = 19;
const RP_SW_POINTER: c_int = 20;
const UPDATE_FRAME: u64 = 1;

#[repr(C)]
struct RenderParam {
    ty: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct RawEvent {
    id: c_int,
    error: c_int,
    reply: u64,
    data: *mut c_void,
}

#[repr(C)]
struct RawProp {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct RawEndFile {
    reason: c_int,
    error: c_int,
}

type Cb = unsafe extern "C" fn(*mut c_void);

/// функции libmpv, найденные по имени
struct Api {
    create: unsafe extern "C" fn() -> *mut c_void,
    initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    terminate_destroy: unsafe extern "C" fn(*mut c_void),
    set_option_string: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    command_async: unsafe extern "C" fn(*mut c_void, u64, *mut *const c_char) -> c_int,
    set_property_async: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int, *mut c_void) -> c_int,
    observe_property: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    wait_event: unsafe extern "C" fn(*mut c_void, f64) -> *mut RawEvent,
    set_wakeup_callback: unsafe extern "C" fn(*mut c_void, Option<Cb>, *mut c_void),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    rc_create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut RenderParam) -> c_int,
    rc_set_update_callback: unsafe extern "C" fn(*mut c_void, Option<Cb>, *mut c_void),
    rc_update: unsafe extern "C" fn(*mut c_void) -> u64,
    rc_render: unsafe extern "C" fn(*mut c_void, *mut RenderParam) -> c_int,
    rc_free: unsafe extern "C" fn(*mut c_void),
}

fn load() -> Result<Api, String> {
    let mut lib = std::ptr::null_mut();
    for name in ["libmpv.so.2", "libmpv.so.1", "libmpv.so"] {
        let c = CString::new(name).unwrap();
        lib = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if !lib.is_null() {
            break;
        }
    }
    if lib.is_null() {
        return Err("libmpv".into());
    }
    macro_rules! sym {
        ($n:literal) => {{
            let c = CString::new($n).unwrap();
            let p = unsafe { libc::dlsym(lib, c.as_ptr()) };
            if p.is_null() {
                return Err(format!("libmpv: {}", $n));
            }
            // SAFETY: сигнатура — из client.h / render.h
            unsafe { std::mem::transmute::<*mut c_void, _>(p) }
        }};
    }
    Ok(Api {
        create: sym!("mpv_create"),
        initialize: sym!("mpv_initialize"),
        terminate_destroy: sym!("mpv_terminate_destroy"),
        set_option_string: sym!("mpv_set_option_string"),
        command_async: sym!("mpv_command_async"),
        set_property_async: sym!("mpv_set_property_async"),
        observe_property: sym!("mpv_observe_property"),
        wait_event: sym!("mpv_wait_event"),
        set_wakeup_callback: sym!("mpv_set_wakeup_callback"),
        error_string: sym!("mpv_error_string"),
        rc_create: sym!("mpv_render_context_create"),
        rc_set_update_callback: sym!("mpv_render_context_set_update_callback"),
        rc_update: sym!("mpv_render_context_update"),
        rc_render: sym!("mpv_render_context_render"),
        rc_free: sym!("mpv_render_context_free"),
    })
}

/// значение наблюдаемого свойства
#[derive(Clone, Debug)]
pub enum Val {
    None,
    Str(String),
    Flag(bool),
    Int(i64),
    Num(f64),
}

pub enum Event {
    Prop(String, Val),
    FileLoaded,
    VideoReconfig,
    /// файл кончился: ошибка, если была
    EndFile(Option<String>),
    Shutdown,
}

pub struct Mpv {
    api: Api,
    h: *mut c_void,
    rc: *mut c_void,
}

impl Mpv {
    /// mpv без своего окна, ввода и конфигов: всё управление — наше.
    /// `wake` зовётся из нитей mpv, когда есть события или новый кадр.
    pub fn new(opts: &[(&str, &str)], wake: Box<dyn Fn() + Send + Sync>) -> Result<Mpv, String> {
        let api = load()?;
        let h = unsafe { (api.create)() };
        if h.is_null() {
            return Err("mpv_create".into());
        }
        let mut m = Mpv { api, h, rc: std::ptr::null_mut() };
        for (k, v) in opts {
            m.option(k, v);
        }
        let e = unsafe { (m.api.initialize)(h) };
        if e < 0 {
            return Err(m.err(e));
        }
        // будильник живёт до конца процесса: mpv может позвать его до самого destroy
        let wake: &'static (dyn Fn() + Send + Sync) = Box::leak(wake);
        let ctx = Box::leak(Box::new(wake)) as *mut &'static (dyn Fn() + Send + Sync) as *mut c_void;
        unsafe extern "C" fn cb(d: *mut c_void) {
            let f = &*(d as *const &'static (dyn Fn() + Send + Sync));
            f();
        }
        let api_type = CString::new("sw").unwrap();
        let mut params = [RenderParam { ty: RP_API_TYPE, data: api_type.as_ptr() as *mut c_void }, RenderParam { ty: RP_INVALID, data: std::ptr::null_mut() }];
        let mut rc = std::ptr::null_mut();
        let e = unsafe { (m.api.rc_create)(&mut rc, h, params.as_mut_ptr()) };
        if e < 0 {
            return Err(m.err(e));
        }
        m.rc = rc;
        unsafe {
            (m.api.set_wakeup_callback)(h, Some(cb), ctx);
            (m.api.rc_set_update_callback)(rc, Some(cb), ctx);
        }
        Ok(m)
    }

    pub fn err(&self, e: c_int) -> String {
        unsafe { CStr::from_ptr((self.api.error_string)(e)) }.to_string_lossy().to_string()
    }

    pub fn option(&mut self, k: &str, v: &str) {
        let (Ok(k), Ok(v)) = (CString::new(k), CString::new(v)) else { return };
        unsafe { (self.api.set_option_string)(self.h, k.as_ptr(), v.as_ptr()) };
    }

    /// команда без ожидания (никогда не держит окно)
    pub fn cmd(&self, args: &[&str]) {
        let cs: Vec<CString> = args.iter().filter_map(|a| CString::new(*a).ok()).collect();
        if cs.len() != args.len() {
            return;
        }
        let mut ptrs: Vec<*const c_char> = cs.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        unsafe { (self.api.command_async)(self.h, 0, ptrs.as_mut_ptr()) };
    }

    pub fn set(&self, k: &str, v: &str) {
        let (Ok(k), Ok(v)) = (CString::new(k), CString::new(v)) else { return };
        let mut p = v.as_ptr();
        unsafe { (self.api.set_property_async)(self.h, 0, k.as_ptr(), FMT_STRING, &mut p as *mut *const c_char as *mut c_void) };
    }

    pub fn observe(&self, k: &str, fmt: char) {
        let Ok(c) = CString::new(k) else { return };
        let f = match fmt {
            's' => FMT_STRING,
            'b' => FMT_FLAG,
            'i' => FMT_INT64,
            _ => FMT_DOUBLE,
        };
        unsafe { (self.api.observe_property)(self.h, 0, c.as_ptr(), f) };
    }

    /// следующее событие без ожидания
    pub fn event(&self) -> Option<Event> {
        loop {
            let e = unsafe { &*(self.api.wait_event)(self.h, 0.0) };
            match e.id {
                EV_NONE => return None,
                EV_SHUTDOWN => return Some(Event::Shutdown),
                EV_FILE_LOADED => return Some(Event::FileLoaded),
                EV_VIDEO_RECONFIG => return Some(Event::VideoReconfig),
                EV_END_FILE => {
                    let d = e.data as *const RawEndFile;
                    // 4 — MPV_END_FILE_REASON_ERROR
                    let err = if d.is_null() { None } else { unsafe { ((*d).reason == 4).then(|| self.err((*d).error)) } };
                    return Some(Event::EndFile(err));
                }
                EV_PROPERTY_CHANGE => {
                    let p = unsafe { &*(e.data as *const RawProp) };
                    let name = unsafe { CStr::from_ptr(p.name) }.to_string_lossy().to_string();
                    let v = unsafe {
                        if p.data.is_null() {
                            Val::None
                        } else {
                            match p.format {
                                FMT_STRING => Val::Str(CStr::from_ptr(*(p.data as *const *const c_char)).to_string_lossy().to_string()),
                                FMT_FLAG => Val::Flag(*(p.data as *const c_int) != 0),
                                FMT_INT64 => Val::Int(*(p.data as *const i64)),
                                FMT_DOUBLE => Val::Num(*(p.data as *const f64)),
                                _ => Val::None,
                            }
                        }
                    };
                    return Some(Event::Prop(name, v));
                }
                _ => continue,
            }
        }
    }

    /// есть ли новый кадр (сбрасывает флаг у mpv)
    pub fn frame_ready(&self) -> bool {
        unsafe { (self.api.rc_update)(self.rc) & UPDATE_FRAME != 0 }
    }

    /// нарисовать текущий кадр в буфер XRGB8888 (порядок байт b,g,r,x — «bgr0»), не дожидаясь
    /// его времени: темп задают кадры композитора
    pub fn render(&self, buf: &mut [u32], w: i32, h: i32) {
        if w <= 0 || h <= 0 || buf.len() < (w * h) as usize {
            return;
        }
        let mut size = [w as c_int, h as c_int];
        let fmt = c"bgr0";
        let mut stride: usize = w as usize * 4;
        let mut block: c_int = 0;
        let mut params = [
            RenderParam { ty: RP_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            RenderParam { ty: RP_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            RenderParam { ty: RP_SW_STRIDE, data: &mut stride as *mut usize as *mut c_void },
            RenderParam { ty: RP_SW_POINTER, data: buf.as_mut_ptr() as *mut c_void },
            RenderParam { ty: RP_BLOCK_FOR_TARGET_TIME, data: &mut block as *mut c_int as *mut c_void },
            RenderParam { ty: RP_INVALID, data: std::ptr::null_mut() },
        ];
        unsafe { (self.api.rc_render)(self.rc, params.as_mut_ptr()) };
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        unsafe {
            if !self.rc.is_null() {
                (self.api.rc_set_update_callback)(self.rc, None, std::ptr::null_mut());
                (self.api.rc_free)(self.rc);
            }
            (self.api.set_wakeup_callback)(self.h, None, std::ptr::null_mut());
            (self.api.terminate_destroy)(self.h);
        }
    }
}
