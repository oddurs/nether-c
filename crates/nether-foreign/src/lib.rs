//! Stratum 8: foreign code, and the only `unsafe` in the language.
//!
//! `spec/09-prelude.md` §9.8 and §9.8.1, and `spec/01-strata.md` §1.7. A crate
//! rather than a module, because the workspace sets `unsafe_code = "forbid"`
//! and `forbid` exists so that it cannot be locally undone.
//!
//! `unsafe` lives only where the language ends, and there are two ends. This
//! is where Nether C calls out; `nether-wasm` is where something else calls
//! in. Those two crates, and no others.
//!
//! Nothing here is safe and nothing here pretends to be. §9.8.1 says as much
//! in the specification: a callee that keeps a pointer after it returns has
//! kept a pointer to something it does not own, and no wording stops it. That
//! is what stratum 8 *is*, and it is why §1.7 marks the trace rather than
//! trying to make the call safe.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

#[repr(C)]
struct DlInfo {
    fname: *const c_char,
    fbase: *mut c_void,
    sname: *const c_char,
    saddr: *mut c_void,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[repr(C)]
struct LinkMap {
    addr: usize,
    name: *const c_char,
    ld: *mut c_void,
    next: *mut LinkMap,
    prev: *mut LinkMap,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const RTLD_DI_LINKMAP: c_int = 2;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const RTLD_DL_LINKMAP: c_int = 2;

// `dlopen`, `dlsym`, `dlclose`, `dlerror`, `dladdr` (and glibc Linux `dlinfo`/`dladdr1`).
// Minimal POSIX dynamic-loader declarations without an external binding crate.
#[cfg_attr(target_os = "linux", link(name = "dl"))]
unsafe extern "C" {
    fn dlopen(file: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn dladdr1(
        addr: *const c_void,
        info: *mut DlInfo,
        extra_info: *mut *mut c_void,
        flags: c_int,
    ) -> c_int;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn dlinfo(handle: *mut c_void, request: c_int, info: *mut c_void) -> c_int;
}

/// `RTLD_NOW | RTLD_LOCAL`. Now, so a missing symbol is an error at load
/// rather than a crash at the call; local, so one object's symbols cannot
/// answer for another's by accident. On macOS `RTLD_LOCAL` is `0x4` and
/// omitting it defaults to `RTLD_GLOBAL`; on 32-bit Android `RTLD_NOW` and
/// `RTLD_LOCAL` are both `0x0` (`0x2` is `RTLD_GLOBAL`).
#[cfg(target_os = "macos")]
const NOW_AND_LOCAL: c_int = 0x2 | 0x4;
#[cfg(all(target_os = "android", target_pointer_width = "32"))]
const NOW_AND_LOCAL: c_int = 0x0;
#[cfg(not(any(target_os = "macos", all(target_os = "android", target_pointer_width = "32"))))]
const NOW_AND_LOCAL: c_int = 0x2;

/// Process-global serialization for stratum-8 calls so duplicate `--load`
/// paths or symlink aliases to the same shared object cannot race.
static CALL_LOCK: Mutex<()> = Mutex::new(());

/// Valid non-dangling address passed when `args` is empty, avoiding Rust's
/// `NonNull::dangling()` (`0x1`) which triggers UB in C `memcpy`/`memcmp`.
static EMPTY_ARG: u8 = 0;

/// §9.8.1's signature, as Rust spells it.
type Foreign = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;

/// One shared object, kept open for as long as symbols may be called in it.
///
/// Calls into it take turns. §9.8.1 promises a callee nothing about threads,
/// so two at once would be a race in code nobody here can see.
pub struct Object {
    handle: Mutex<Handle>,
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    canonical: PathBuf,
    /// What it was loaded from, for saying which object answered.
    pub path: String,
}

/// What `dlopen` returned.
struct Handle(*mut c_void);

// SAFETY: a `dlopen` handle names the object for the whole process, and
// `dlsym` and `dlclose` may be called on it from any thread. Calling into the
// object from two threads at once is serialized by `CALL_LOCK` and `handle`.
unsafe impl Send for Handle {}

/// What went wrong, in the words `dlerror` used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed(pub String);

impl core::fmt::Display for Failed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for Failed {}

/// Whatever `dlerror` has to say, or a sentence saying it said nothing.
fn last_error() -> String {
    // SAFETY: `dlerror` returns a pointer to a static buffer or null, and the
    // string is valid until the next call on this thread. It is copied here.
    let said = unsafe { dlerror() };
    if said.is_null() {
        return "the loader gave no reason".to_owned();
    }
    // SAFETY: non-null, and `dlerror` returns a NUL-terminated string.
    unsafe { CStr::from_ptr(said) }.to_string_lossy().into_owned()
}

impl Object {
    /// Open one shared object. §8.3.3: the invocation said which.
    ///
    /// Canonicalizes `path` on every platform before `dlopen` so bare system
    /// library names (such as `libc.so.6`) cannot resolve via ambient loader
    /// search paths.
    ///
    /// # Errors
    ///
    /// [`Failed`] with whatever the loader said, which is the only account of
    /// what happened that exists.
    pub fn open(path: &Path) -> Result<Self, Failed> {
        if path.as_os_str().is_empty() {
            return Err(Failed("shared object path is empty".to_owned()));
        }
        let shown = path.display().to_string();
        let canonical = path.canonicalize().map_err(|e| Failed(format!("{shown}: {e}")))?;
        let meta = std::fs::metadata(&canonical).map_err(|e| Failed(format!("{shown}: {e}")))?;
        if !meta.is_file() {
            return Err(Failed(format!("{shown} is not a regular file")));
        }
        let Ok(name) = CString::new(canonical.as_os_str().as_bytes()) else {
            return Err(Failed(format!("{shown} has a NUL in it")));
        };
        // SAFETY: `name` is a valid NUL-terminated string that outlives the
        // call. Hold `CALL_LOCK` across `dlopen` (which runs `.init_array`) and
        // `last_error` so concurrent opens/calls/drops cannot race.
        let _turn = CALL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let handle = unsafe { dlopen(name.as_ptr(), NOW_AND_LOCAL) };
        if handle.is_null() {
            Err(Failed(last_error()))
        } else {
            Ok(Self {
                handle: Mutex::new(Handle(handle)),
                #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
                canonical,
                path: shown,
            })
        }
    }

    /// Whether this object has that symbol.
    #[must_use]
    pub fn has(&self, symbol: &str) -> bool {
        let _turn = CALL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        self.find(&handle, symbol).is_some()
    }

    /// Verify that `found` is defined in the opened shared object itself, not
    /// resolved transitively through `DT_NEEDED` dependencies such as `libc`.
    fn defined_in_object(&self, handle: &Handle, found: *mut c_void) -> bool {
        let mut info = DlInfo {
            fname: core::ptr::null(),
            fbase: core::ptr::null_mut(),
            sname: core::ptr::null(),
            saddr: core::ptr::null_mut(),
        };
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let _ = self;
            let mut handle_lm: *mut LinkMap = core::ptr::null_mut();
            let mut sym_lm: *mut c_void = core::ptr::null_mut();
            // Compare in-memory `link_map *` pointers directly so filesystem
            // hardlink TOCTOU, atomic `.so` replacement, and non-zero lowest
            // `PT_LOAD` `p_vaddr` offsets cannot mislead ownership checks.
            // SAFETY: `handle.0` is an open `dlopen` handle and `found` is a
            // valid symbol address returned by `dlsym`.
            let got_handle =
                unsafe { dlinfo(handle.0, RTLD_DI_LINKMAP, (&raw mut handle_lm).cast()) } == 0
                    && !handle_lm.is_null();
            let got_sym = unsafe {
                dladdr1(found.cast_const(), &raw mut info, &raw mut sym_lm, RTLD_DL_LINKMAP)
            } != 0
                && !sym_lm.is_null();
            got_handle && got_sym && sym_lm.cast_const() == handle_lm.cast_const().cast()
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = handle;
            // SAFETY: `found` is a valid code address returned by `dlsym`.
            if unsafe { dladdr(found.cast_const(), &raw mut info) } == 0 {
                return false;
            }
            if !info.fname.is_null() {
                // SAFETY: `fname` is a non-null NUL-terminated path from `dladdr`.
                let fname = unsafe { CStr::from_ptr(info.fname) };
                let fname_path = Path::new(std::ffi::OsStr::from_bytes(fname.to_bytes()));
                // Prefer exact canonical path match before touching the
                // filesystem so atomic `.so` replacement keeps working and
                // hardlinks at another path cannot impersonate `self`.
                if fname_path == self.canonical.as_path() {
                    return true;
                }
                if fname_path.canonicalize().is_ok_and(|p| p == self.canonical) {
                    return true;
                }
            }
            false
        }
    }

    /// The symbol, if this object itself defines it.
    fn find(&self, handle: &Handle, symbol: &str) -> Option<Foreign> {
        let name = CString::new(symbol).ok()?;
        // SAFETY: `handle` came from `dlopen` and has not been closed; `name`
        // is NUL-terminated and outlives the call.
        let found = unsafe { dlsym(handle.0, name.as_ptr()) };
        if found.is_null() || !self.defined_in_object(handle, found) {
            return None;
        }
        // SAFETY: this is the transmute stratum 8 is about. Nothing checks
        // that the symbol has §9.8.1's signature, and nothing can: a shared
        // object carries no types. Calling it is undefined behaviour if it
        // does not, which is why a trace that reached here is marked forever.
        Some(unsafe { core::mem::transmute::<*mut c_void, Foreign>(found) })
    }

    /// Call it, by §9.8.1's protocol.
    ///
    /// Twice at most: `1` means the buffer was too small and `*out_len` is
    /// what is needed, and a callee whose answer grows every time it is asked
    /// is a callee that never finishes.
    ///
    /// `Ok((code, bytes))` is the callee's return value and what it wrote.
    ///
    /// # Errors
    ///
    /// [`Failed`] if this object does not have that symbol, or if it asked
    /// twice for more room.
    pub fn call(&self, symbol: &str, args: &[u8]) -> Result<(i32, Vec<u8>), Failed> {
        const MAX_OUTPUT: usize = 64 * 1024 * 1024;
        // Held to the end of the call, both tries, across every open handle.
        let _turn = CALL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(function) = self.find(&handle, symbol) else {
            return Err(Failed(format!("{} has no `{symbol}`", self.path)));
        };
        let args_ptr = if args.is_empty() { &raw const EMPTY_ARG } else { args.as_ptr() };
        // Enough for most answers without asking, and small enough that being
        // wrong costs one more call rather than a reservation.
        let mut out = vec![0u8; 4096];
        let mut wrote = 0usize;
        // SAFETY: none available. `args_ptr` and `out` are valid non-dangling
        // pointers live for the call and their lengths are what is passed.
        let code =
            unsafe { function(args_ptr, args.len(), out.as_mut_ptr(), out.len(), &raw mut wrote) };
        if code != 1 {
            if code == 0 && wrote > out.len() {
                return Err(Failed(format!(
                    "`{symbol}` claimed to write {wrote} bytes into a {}-byte buffer",
                    out.len()
                )));
            }
            out.truncate(wrote.min(out.len()));
            return Ok((code, out));
        }

        if wrote > MAX_OUTPUT {
            return Err(Failed(format!(
                "`{symbol}` asked for {wrote} bytes, past the 64 MiB bound"
            )));
        }
        let alloc_len = wrote.max(1);
        let mut out = Vec::new();
        out.try_reserve_exact(alloc_len)
            .map_err(|_| Failed(format!("`{symbol}` could not reserve {wrote} bytes")))?;
        out.resize(alloc_len, 0);
        let cap = wrote;
        let mut wrote = 0usize;
        // SAFETY: as above, with a non-dangling buffer of at least `cap` bytes.
        let code = unsafe { function(args_ptr, args.len(), out.as_mut_ptr(), cap, &raw mut wrote) };
        if code == 1 {
            return Err(Failed(format!("`{symbol}` asked for more room twice")));
        }
        if code == 0 && wrote > cap {
            return Err(Failed(format!(
                "`{symbol}` claimed to write {wrote} bytes into a {cap}-byte buffer"
            )));
        }
        out.truncate(wrote.min(cap));
        Ok((code, out))
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        let _turn = CALL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let handle = self.handle.get_mut().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `handle` came from `dlopen` and is closed once, here.
        unsafe { dlclose(handle.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::Object;
    use std::path::Path;

    #[test]
    fn empty_path_is_refused_before_dlopen() {
        let err = Object::open(Path::new("")).err().expect("empty path must fail");
        assert!(err.0.contains("empty"), "{err}");
    }

    #[test]
    fn bare_system_library_name_is_refused_before_dlopen() {
        for name in ["libc.so.6", "libSystem.B.dylib"] {
            assert!(
                Object::open(Path::new(name)).is_err(),
                "bare ambient library name {name:?} must not open via loader search paths"
            );
        }
    }
}
