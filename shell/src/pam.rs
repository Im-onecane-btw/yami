// Проверка пароля через PAM напрямую (libpam), без лишних crate'ов и bindgen. Свой файл
// сервиса лежит в ~/.local/state/yami/pam: только pam_unix, без журналов входа.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

#[repr(C)]
struct PamConv {
    conv: extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int,
    appdata_ptr: *mut c_void,
}

#[link(name = "pam")]
extern "C" {
    fn pam_start_confdir(service: *const c_char, user: *const c_char, conv: *const PamConv, confdir: *const c_char, pamh: *mut *mut c_void) -> c_int;
    fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut c_void, status: c_int) -> c_int;
}

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

extern "C" fn conv(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
    unsafe {
        let arr = libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if arr.is_null() {
            return PAM_BUF_ERR;
        }
        let pass = data as *const c_char;
        for i in 0..n as isize {
            let m = *msgs.offset(i);
            let style = (*m).msg_style;
            if style == PAM_PROMPT_ECHO_OFF || style == PAM_PROMPT_ECHO_ON {
                (*arr.offset(i)).resp = libc::strdup(pass);
            }
        }
        *resp = arr;
    }
    PAM_SUCCESS
}

const SERVICE: &str = "# yami lock: password check only\nauth required pam_unix.so\n";

pub fn check(password: &str) -> bool {
    let dir = crate::config::state_dir().join("pam");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("yami-lock");
    if std::fs::read_to_string(&f).map(|t| t != SERVICE).unwrap_or(true) {
        let _ = std::fs::write(&f, SERVICE);
    }
    let user = unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() {
            return false;
        }
        CStr::from_ptr((*pw).pw_name).to_owned()
    };
    let (Ok(svc), Ok(pass), Ok(cdir)) = (CString::new("yami-lock"), CString::new(password), CString::new(dir.to_string_lossy().as_bytes())) else { return false };
    let c = PamConv { conv, appdata_ptr: pass.as_ptr() as *mut c_void };
    let mut h: *mut c_void = std::ptr::null_mut();
    unsafe {
        if pam_start_confdir(svc.as_ptr(), user.as_ptr(), &c, cdir.as_ptr(), &mut h) != PAM_SUCCESS {
            return false;
        }
        let r = pam_authenticate(h, 0);
        pam_end(h, r);
        // копия пароля для PAM — затереть
        let mut b = pass.into_bytes();
        std::ptr::write_bytes(b.as_mut_ptr(), 0, b.len());
        r == PAM_SUCCESS
    }
}
