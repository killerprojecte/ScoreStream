//! The only unsafe boundary. No caller-supplied executable/library path.
use crate::{AudioError, BackendCapabilities, RenderOptions, Result, RouteAssignment};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    fs::{self, File},
    io::{Read, Seek},
    marker::PhantomData,
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

type Ptr = *mut c_void;
type New = unsafe extern "C" fn() -> Ptr;
type Delete = unsafe extern "C" fn(Ptr);
type SetInt = unsafe extern "C" fn(Ptr, *const c_char, c_int) -> c_int;
type SetNum = unsafe extern "C" fn(Ptr, *const c_char, f64) -> c_int;
type GetInt = unsafe extern "C" fn(Ptr) -> c_int;
type GetName = unsafe extern "C" fn(Ptr) -> *const c_char;
type Next = unsafe extern "C" fn(Ptr) -> Ptr;
type Message3 = unsafe extern "C" fn(Ptr, c_int, c_int, c_int) -> c_int;
type Message2 = unsafe extern "C" fn(Ptr, c_int, c_int) -> c_int;

struct Library {
    handle: NonNull<c_void>,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Library {
    fn open(path: &Path) -> Result<Self> {
        let path = c_path(path)?;
        // SAFETY: verified fixed system path is NUL-terminated; the OS loader
        // owns the returned handle. No symbol or instance outlives this owner.
        // FluidSynth's dependencies contain process-global type registries.
        // Keep code and dependency statics resident; unloading/reloading them
        // can corrupt those registries even after all synths have been deleted.
        let handle = unsafe {
            libc::dlopen(
                path.as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL | libc::RTLD_NODELETE,
            )
        };
        Ok(Self {
            handle: NonNull::new(handle).ok_or_else(|| AudioError::Unavailable(dl_error()))?,
            _thread_bound: PhantomData,
        })
    }
    fn symbol(&self, name: &CStr) -> Result<Ptr> {
        // SAFETY: handle is live, name is NUL-terminated. Only required known
        // FluidSynth-2 ABI symbols are requested below.
        let pointer = unsafe { libc::dlsym(self.handle.as_ptr(), name.as_ptr()) };
        if pointer.is_null() {
            return Err(AudioError::Unavailable(format!(
                "Missing native symbol {}: {}",
                name.to_string_lossy(),
                dl_error()
            )));
        }
        Ok(pointer)
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: release this handle after all Synth borrowers are gone.
        // RTLD_NODELETE intentionally keeps code/dependencies resident until exit.
        unsafe {
            libc::dlclose(self.handle.as_ptr());
        }
    }
}
fn dl_error() -> String {
    // SAFETY: dlerror returns a thread-local NUL-terminated string or null.
    unsafe {
        let value = libc::dlerror();
        if value.is_null() {
            "Native loader did not supply an error".into()
        } else {
            CStr::from_ptr(value).to_string_lossy().into_owned()
        }
    }
}
fn c_path(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AudioError::Unavailable("Path contains NUL".into()))
}

pub(crate) fn hash_file(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    file.rewind()?;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    file.rewind()?;
    Ok(format!("{:x}", hash.finalize()))
}

/// The system image's /usr ownership is the trust anchor (UID 0 on ordinary
/// installs; this managed image uses an immutable mapped owner). Reject any
/// current-user-owned, group/other-writable, or differently-owned path component.
/// This is a constrained loader, NOT a sandbox against a compromised OS/root.
fn verify_system_path(path: &Path, anchor: &Path) -> Result<()> {
    let root = fs::metadata(anchor)?;
    // SAFETY: geteuid is a side-effect-free process identity query.
    let current = unsafe { libc::geteuid() };
    let trusted = root.uid();
    if trusted == current || root.mode() & 0o022 != 0 {
        return Err(AudioError::Unavailable(
            "System library trust anchor is writable or user-owned".into(),
        ));
    }
    if !path.starts_with(anchor) {
        return Err(AudioError::Unavailable(
            "Library resolves outside the system root".into(),
        ));
    }
    for ancestor in path.ancestors().take_while(|item| item.starts_with(anchor)) {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink()
            || metadata.uid() == current
            || (metadata.uid() != 0 && metadata.uid() != trusted)
            || metadata.mode() & 0o022 != 0
        {
            return Err(AudioError::Unavailable(
                "System library path has an untrusted mutable component".into(),
            ));
        }
    }
    Ok(())
}

fn installed_library_path() -> Result<PathBuf> {
    if std::env::vars_os()
        .any(|(key, value)| key.as_bytes().starts_with(b"LD_") && !value.is_empty())
    {
        return Err(AudioError::Unavailable(
            "LD_* loader overrides are set; refusing ambiguous native loading".into(),
        ));
    }
    #[cfg(target_arch = "x86_64")]
    const CANDIDATES: &[&str] = &[
        "/usr/lib/x86_64-linux-gnu/libfluidsynth.so.3",
        "/usr/lib64/libfluidsynth.so.3",
        "/usr/lib/libfluidsynth.so.3",
    ];
    #[cfg(target_arch = "aarch64")]
    const CANDIDATES: &[&str] = &[
        "/usr/lib/aarch64-linux-gnu/libfluidsynth.so.3",
        "/usr/lib64/libfluidsynth.so.3",
        "/usr/lib/libfluidsynth.so.3",
    ];
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    const CANDIDATES: &[&str] = &[];
    for candidate in CANDIDATES {
        if !Path::new(candidate).try_exists()? {
            continue;
        }
        let path = Path::new(candidate).canonicalize()?;
        verify_system_path(&path, Path::new("/usr"))?;
        if !path.is_file() {
            return Err(AudioError::Unavailable(
                "Library is not a regular file".into(),
            ));
        }
        let mut file = File::open(&path)?;
        let mut magic = [0_u8; 4];
        file.read_exact(&mut magic)?;
        if magic != *b"\x7fELF" {
            return Err(AudioError::Unavailable("System library is not ELF".into()));
        }
        return Ok(path);
    }
    Err(AudioError::Unavailable(
        "No supported installed FluidSynth-2 ABI library at fixed system locations".into(),
    ))
}

pub(crate) struct Api {
    _library: Library,
    pub capabilities: BackendCapabilities,
    new_settings: New,
    delete_settings: Delete,
    set_int: SetInt,
    set_num: SetNum,
    new_synth: unsafe extern "C" fn(Ptr) -> Ptr,
    delete_synth: Delete,
    sfload: unsafe extern "C" fn(Ptr, *const c_char, c_int) -> c_int,
    get_bank: unsafe extern "C" fn(Ptr, c_int) -> Ptr,
    preset_start: Delete,
    preset_next: Next,
    preset_name: GetName,
    preset_bank: GetInt,
    preset_program: GetInt,
    select: unsafe extern "C" fn(Ptr, c_int, c_int, c_int, c_int) -> c_int,
    get_program: unsafe extern "C" fn(Ptr, c_int, *mut c_int, *mut c_int, *mut c_int) -> c_int,
    channel_type: Message2,
    cc: Message3,
    get_cc: unsafe extern "C" fn(Ptr,c_int,c_int,*mut c_int)->c_int,
    noteon: Message3,
    noteoff: Message2,
    write_float: unsafe extern "C" fn(Ptr, c_int, Ptr, c_int, c_int, Ptr, c_int, c_int) -> c_int,
    quantum: GetInt,
    voices: GetInt,
}

impl Api {
    pub fn open() -> Result<Self> {
        let path = installed_library_path()?;
        let hash = hash_file(&mut File::open(&path)?)?;
        let library = Library::open(&path)?;
        macro_rules! load {
            ($name:literal, $kind:ty) => {{
                let name = CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes())
                    .expect("static ABI name");
                let pointer = library.symbol(name)?;
                // SAFETY: each signature below matches the documented FluidSynth
                // 2 ABI; Library retains the module for the lifetime of Api.
                unsafe { std::mem::transmute::<Ptr, $kind>(pointer) }
            }};
        }
        let version_fn = load!("fluid_version_str", unsafe extern "C" fn() -> *const c_char);
        // SAFETY: known ABI returns a static NUL-terminated version string.
        let version_pointer = unsafe { version_fn() };
        if version_pointer.is_null() {
            return Err(AudioError::Unavailable("Null native version".into()));
        }
        let version = unsafe { CStr::from_ptr(version_pointer) }
            .to_string_lossy()
            .into_owned();
        if !version.starts_with("2.") {
            return Err(AudioError::Unavailable(format!(
                "Unsupported FluidSynth ABI {version}"
            )));
        }
        Ok(Self {
            capabilities: BackendCapabilities {
                id: "fluidsynth-linux-offline-v1".into(),
                version,
                library_path: path,
                checked_library_sha256: hash,
                offline_pcm: true,
                realtime_audio: false,
                midi_device_output: false,
                vst3: false,
                per_note_expression: false,
                microtonal_pitch: false,
                max_routes: 256,
            },
            new_settings: load!("new_fluid_settings", New),
            delete_settings: load!("delete_fluid_settings", Delete),
            set_int: load!("fluid_settings_setint", SetInt),
            set_num: load!("fluid_settings_setnum", SetNum),
            new_synth: load!("new_fluid_synth", unsafe extern "C" fn(Ptr) -> Ptr),
            delete_synth: load!("delete_fluid_synth", Delete),
            sfload: load!(
                "fluid_synth_sfload",
                unsafe extern "C" fn(Ptr, *const c_char, c_int) -> c_int
            ),
            get_bank: load!(
                "fluid_synth_get_sfont_by_id",
                unsafe extern "C" fn(Ptr, c_int) -> Ptr
            ),
            preset_start: load!("fluid_sfont_iteration_start", Delete),
            preset_next: load!("fluid_sfont_iteration_next", Next),
            preset_name: load!("fluid_preset_get_name", GetName),
            preset_bank: load!("fluid_preset_get_banknum", GetInt),
            preset_program: load!("fluid_preset_get_num", GetInt),
            select: load!(
                "fluid_synth_program_select",
                unsafe extern "C" fn(Ptr, c_int, c_int, c_int, c_int) -> c_int
            ),
            get_program: load!(
                "fluid_synth_get_program",
                unsafe extern "C" fn(Ptr, c_int, *mut c_int, *mut c_int, *mut c_int) -> c_int
            ),
            channel_type: load!("fluid_synth_set_channel_type", Message2),
            cc: load!("fluid_synth_cc", Message3),
            get_cc: load!("fluid_synth_get_cc", unsafe extern "C" fn(Ptr,c_int,c_int,*mut c_int)->c_int),
            noteon: load!("fluid_synth_noteon", Message3),
            noteoff: load!("fluid_synth_noteoff", Message2),
            write_float: load!(
                "fluid_synth_write_float",
                unsafe extern "C" fn(Ptr, c_int, Ptr, c_int, c_int, Ptr, c_int, c_int) -> c_int
            ),
            quantum: load!("fluid_synth_get_internal_bufsize", GetInt),
            voices: load!("fluid_synth_get_active_voice_count", GetInt),
            _library: library,
        })
    }
}

fn check(value: c_int, name: &str) -> Result<()> {
    if value != 0 {
        return Err(AudioError::Native(format!("{name}: {value}")));
    }
    Ok(())
}

pub(crate) struct Bank {
    pub path: PathBuf,
    pub hash: String,
    file: File,
    max_bytes: u64,
    fingerprint: (u64, u64, u64, i64, i64, i64, i64),
}
impl Bank {
    pub fn open(path: &Path, options: &RenderOptions, cancel: &AtomicBool) -> Result<Self> {
        let path = path.canonicalize()?;
        let mut file = File::open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.len() < 12
            || metadata.len() > options.max_soundfont_bytes
        {
            return Err(AudioError::InvalidPlan(
                "Bank is not a regular file or exceeds explicit resource budget".into(),
            ));
        }
        let mut header = [0_u8; 12];
        file.read_exact(&mut header)?;
        if header[..4] != *b"RIFF" || header[8..] != *b"sfbk" {
            return Err(AudioError::InvalidPlan(
                "Bank is not a RIFF SoundFont".into(),
            ));
        }
        let fingerprint = Self::fingerprint(&metadata);
        let hash = Self::bounded_hash(&mut file, options.max_soundfont_bytes, cancel)?;
        if Self::fingerprint(&file.metadata()?) != fingerprint {
            return Err(AudioError::InvalidPlan("Bank changed while hashing".into()));
        }
        Ok(Self {
            path,
            hash,
            file,
            max_bytes: options.max_soundfont_bytes,
            fingerprint,
        })
    }

    fn fingerprint(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    }

    fn bounded_hash(file: &mut File, limit: u64, cancel: &AtomicBool) -> Result<String> {
        file.rewind()?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut bytes = 0_u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(AudioError::Cancelled);
            }
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > limit {
                return Err(AudioError::InvalidPlan(
                    "Bank grew beyond its resource budget".into(),
                ));
            }
            hash.update(&buffer[..count]);
        }
        file.rewind()?;
        Ok(format!("{:x}", hash.finalize()))
    }

    fn native_path(&self) -> Result<CString> {
        // Pin sfload to the validated inode, including if the original pathname
        // is replaced or unlinked. This is internal, never a caller library path.
        c_path(Path::new(&format!(
            "/proc/self/fd/{}",
            self.file.as_raw_fd()
        )))
    }

    fn verify_unchanged(&mut self, cancel: &AtomicBool) -> Result<()> {
        if Self::fingerprint(&self.file.metadata()?) != self.fingerprint
            || Self::bounded_hash(&mut self.file, self.max_bytes, cancel)? != self.hash
            || Self::fingerprint(&self.file.metadata()?) != self.fingerprint
        {
            return Err(AudioError::InvalidPlan(
                "Bank changed during native loading".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) struct Synth<'a> {
    api: &'a Api,
    settings: NonNull<c_void>,
    synth: Option<NonNull<c_void>>,
    pub quantum: u32,
    pub names: Vec<String>,
}
impl<'a> Synth<'a> {
    pub fn new(
        api: &'a Api,
        bank: &mut Bank,
        routes: &[RouteAssignment],
        options: &RenderOptions,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        // SAFETY: API is loaded and thread-bound; pointer is owned by this guard.
        let settings = NonNull::new(unsafe { (api.new_settings)() })
            .ok_or_else(|| AudioError::Native("new settings returned null".into()))?;
        let mut result = Self {
            api,
            settings,
            synth: None,
            quantum: 0,
            names: Vec::new(),
        };
        for (name, value) in [
            (c"synth.sample-rate", f64::from(options.sample_rate)),
            (c"synth.gain", options.gain),
        ] {
            check(
                unsafe { (api.set_num)(settings.as_ptr(), name.as_ptr(), value) },
                "set numeric setting",
            )?;
        }
        let channels = routes.len().max(1).div_ceil(16) * 16;
        for (name, value) in [
            (c"synth.reverb.active", 0),
            (c"synth.chorus.active", 0),
            (c"synth.cpu-cores", 1),
            (c"synth.polyphony", options.polyphony as i32),
            (c"synth.midi-channels", channels as i32),
            (c"synth.min-note-length", 0),
            (c"synth.lock-memory", 0),
            (c"synth.dynamic-sample-loading", 0),
        ] {
            check(
                unsafe { (api.set_int)(settings.as_ptr(), name.as_ptr(), value) },
                "set integer setting",
            )?;
        }
        result.synth = Some(
            NonNull::new(unsafe { (api.new_synth)(settings.as_ptr()) })
                .ok_or_else(|| AudioError::Native("new synth returned null".into()))?,
        );
        let synth = result.pointer();
        let bank_path = bank.native_path()?;
        // Do not reset unrelated channels or select default GM/drum patches.
        // Every used route is explicitly assigned below after enumeration.
        let id = unsafe { (api.sfload)(synth, bank_path.as_ptr(), 0) };
        if id < 0 {
            return Err(AudioError::Native("Cannot load selected bank".into()));
        }
        bank.verify_unchanged(cancel)?;
        let loaded_bank = unsafe { (api.get_bank)(synth, id) };
        if loaded_bank.is_null() {
            return Err(AudioError::Native("No loaded bank handle".into()));
        }
        let mut presets = BTreeMap::new();
        unsafe {
            (api.preset_start)(loaded_bank);
        }
        loop {
            let preset = unsafe { (api.preset_next)(loaded_bank) };
            if preset.is_null() {
                break;
            }
            let name = unsafe { (api.preset_name)(preset) };
            if name.is_null() {
                return Err(AudioError::Native("Null preset name".into()));
            }
            let key = unsafe { ((api.preset_bank)(preset), (api.preset_program)(preset)) };
            presets.insert(
                key,
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for (channel, route) in routes.iter().enumerate() {
            let patch = (i32::from(route.bank), i32::from(route.program));
            let name = presets.get(&patch).ok_or_else(|| {
                AudioError::Unsupported(format!("Selected bank lacks exact preset {patch:?}"))
            })?;
            // Every route is explicit; channel 10 must not silently become drums.
            check(
                unsafe { (api.channel_type)(synth, channel as i32, 0) },
                "channel type",
            )?;
            check(
                unsafe { (api.select)(synth, channel as i32, id, patch.0, patch.1) },
                "select patch",
            )?;
            let (mut actual_id, mut actual_bank, mut actual_program) = (0, 0, 0);
            check(
                unsafe {
                    (api.get_program)(
                        synth,
                        channel as i32,
                        &mut actual_id,
                        &mut actual_bank,
                        &mut actual_program,
                    )
                },
                "read selected patch",
            )?;
            if (actual_id, actual_bank, actual_program) != (id, patch.0, patch.1) {
                return Err(AudioError::Native(
                    "Native engine substituted a different patch".into(),
                ));
            }
            if let Some(mix)=&options.local_midi_mix {
                mix.verify_routes(routes)?;
                mix.verify_bank(&bank.hash)?;
                let receipt=&mix.routes()[channel];
                for (controller,value) in [(7,receipt.volume.as_ref().map(|v|v.cc7)),(10,receipt.cc10)] {
                    if let Some(value)=value {
                        // SAFETY: valid live synth, validated local channel and 7-bit CC value.
                        check(unsafe {(api.cc)(synth,channel as i32,controller,i32::from(value))},"set source MIDI controller")?;
                        let mut actual=0;
                        check(unsafe {(api.get_cc)(synth,channel as i32,controller,&mut actual)},"read source MIDI controller")?;
                        if actual!=i32::from(value){return Err(AudioError::Native("Native controller readback mismatch".into()));}
                    }
                }
            }
            result.names.push(name.clone());
        }
        let quantum = unsafe { (api.quantum)(synth) };
        if quantum <= 0 {
            return Err(AudioError::Native("Invalid synthesis quantum".into()));
        }
        result.quantum = quantum as u32;
        Ok(result)
    }
    fn pointer(&self) -> Ptr {
        self.synth.expect("initialized native synth").as_ptr()
    }
    pub fn event(&mut self, note: &crate::plan::Note, on: bool) -> Result<bool> {
        let channel = i32::from(note.channel);
        let key = i32::from(note.key);
        if on {
            check(
                unsafe {
                    (self.api.noteon)(self.pointer(), channel, key, i32::from(note.velocity))
                },
                "note on",
            )?;
            Ok(false)
        } else {
            let code = unsafe { (self.api.noteoff)(self.pointer(), channel, key) };
            // Documented -1 also means a naturally completed sample voice.
            match code {
                0 => Ok(false),
                -1 => Ok(true),
                _ => {
                    check(code, "note off")?;
                    Ok(false)
                }
            }
        }
    }
    pub fn voices(&self) -> Result<u32> {
        let count = unsafe { (self.api.voices)(self.pointer()) };
        u32::try_from(count).map_err(|_| AudioError::Native("Negative voice count".into()))
    }
    pub fn render(&mut self, samples: &mut [f32]) -> Result<()> {
        if samples.len() % 2 != 0 || samples.len() / 2 > i32::MAX as usize {
            return Err(AudioError::InvalidPlan("Invalid stereo block".into()));
        }
        let data = samples.as_mut_ptr().cast::<c_void>();
        // SAFETY: output buffers reference the same allocated slice, with
        // non-overlapping even/odd channel indices and exact frame count.
        check(
            unsafe {
                (self.api.write_float)(
                    self.pointer(),
                    (samples.len() / 2) as i32,
                    data,
                    0,
                    2,
                    data,
                    1,
                    2,
                )
            },
            "render PCM",
        )
    }
}
impl Drop for Synth<'_> {
    fn drop(&mut self) {
        // SAFETY: these handles are unique, thread-bound, and belong to api.
        unsafe {
            if let Some(synth) = self.synth {
                (self.api.delete_synth)(synth.as_ptr());
            }
            (self.api.delete_settings)(self.settings.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_user_owned_library_root() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake.so");
        fs::write(&path, b"\x7fELF").unwrap();
        assert!(verify_system_path(&path, dir.path()).is_err());
    }
    #[test]
    fn refuses_path_outside_system_anchor() {
        assert!(verify_system_path(Path::new("/tmp/fake.so"), Path::new("/usr")).is_err());
    }

    #[test]
    fn bank_handle_is_pinned_and_mutations_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selected.sf2");
        let old = dir.path().join("original.sf2");
        let bytes = b"RIFF\x08\x00\x00\x00sfbkdata";
        fs::write(&path, bytes).unwrap();
        let cancel = AtomicBool::new(false);
        let mut bank = Bank::open(&path, &RenderOptions::default(), &cancel).unwrap();
        fs::rename(&path, &old).unwrap();
        fs::write(&path, b"replacement").unwrap();
        let fd_path = bank.native_path().unwrap();
        assert_eq!(
            fs::read(Path::new(std::ffi::OsStr::from_bytes(fd_path.as_bytes()))).unwrap(),
            bytes
        );
        // Renaming the original may change ctime, intentionally rejected by the
        // provenance guard despite the pinned loader still seeing original bytes.
        fs::write(&old, b"RIFF\x08\x00\x00\x00sfbkedit").unwrap();
        assert!(bank.verify_unchanged(&cancel).is_err());
    }

    #[test]
    fn bank_hashing_obeys_cancellation_and_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selected.sf2");
        fs::write(&path, b"RIFF\x08\x00\x00\x00sfbkdata").unwrap();
        let mut file = File::open(&path).unwrap();
        assert!(matches!(
            Bank::bounded_hash(&mut file, 100, &AtomicBool::new(true)),
            Err(AudioError::Cancelled)
        ));
        assert!(Bank::bounded_hash(&mut file, 12, &AtomicBool::new(false)).is_err());
    }
}
