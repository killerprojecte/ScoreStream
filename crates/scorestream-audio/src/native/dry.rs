//! Internal opt-in dry-group primitive. No ordinary backend/API behavior changes.
//! One admitted channel per group; synchronous offline rendering only. No MIDI
//! driver, FX, concurrent calls, modulo channel aliases, or guessed tail timeout.
use super::*;

type Process =
    unsafe extern "C" fn(Ptr, c_int, c_int, *mut *mut f32, c_int, *mut *mut f32) -> c_int;
type VoiceList = unsafe extern "C" fn(Ptr, *mut Ptr, c_int, c_int);
struct DryApi {
    process: Process,
    groups: GetInt,
    channels: GetInt,
    polyphony: GetInt,
    list: VoiceList,
    playing: GetInt,
    channel: GetInt,
    settings_int: unsafe extern "C" fn(Ptr, *const c_char, *mut c_int) -> c_int,
}
impl DryApi {
    fn load(api: &Api) -> Result<Self> {
        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                let name = CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes()).unwrap();
                // SAFETY: public FluidSynth 2 signatures, module pinned by Api;
                // this private table never outlives its Synth's Api borrower.
                unsafe { std::mem::transmute::<Ptr, $ty>(api._library.symbol(name)?) }
            }};
        }
        Ok(Self {
            process: sym!("fluid_synth_process", Process),
            groups: sym!("fluid_synth_count_audio_groups", GetInt),
            channels: sym!("fluid_synth_count_audio_channels", GetInt),
            polyphony: sym!("fluid_synth_get_polyphony", GetInt),
            list: sym!("fluid_synth_get_voicelist", VoiceList),
            playing: sym!("fluid_voice_is_playing", GetInt),
            channel: sym!("fluid_voice_get_channel", GetInt),
            settings_int: sym!(
                "fluid_settings_getint",
                unsafe extern "C" fn(Ptr, *const c_char, *mut c_int) -> c_int
            ),
        })
    }
}

/// Load the exact dry ABI table and query native limits without a synth or bank.
pub(crate) fn probe_api(api: &Api) -> Result<usize> {
    let _functions = DryApi::load(api)?;
    DrySynth::physical_group_limit(api)
}

/// Private fields prevent callers constructing a fake empty-voice certificate.
/// Valid only at its completed frame; no native pointers escape a snapshot.
#[derive(Debug)]
pub(crate) struct VoiceSnapshot {
    frame: u64,
    playing_per_channel: Vec<usize>,
}
impl VoiceSnapshot {
    pub(crate) fn frame(&self) -> u64 {
        self.frame
    }
    pub(crate) fn playing_per_channel(&self) -> &[usize] {
        &self.playing_per_channel
    }
}

/// Actual initial native controller values, including defaults for source-absent
/// fields. Original authored absence remains solely in the sealed source mixer.
#[derive(Clone, Debug, serde::Serialize, PartialEq, Eq)]
pub(crate) struct RouteBinding {
    pub route_index: usize,
    pub bank: i32,
    pub program: i32,
    pub cc7: i32,
    pub cc10: i32,
    bank_id: i32,
}

pub(crate) struct DrySynth<'a> {
    synth: Synth<'a>,
    functions: DryApi,
    groups: usize,
    frame: u64,
    source_sha256: String,
    owners: Vec<Option<(String, u8, bool)>>,
    authorized_bindings: Vec<RouteBinding>,
    lane_bindings: Vec<usize>,
}
impl<'a> DrySynth<'a> {
    pub(crate) fn new(
        api: &'a Api,
        bank: &mut Bank,
        source: &str,
        routes: &[RouteAssignment],
        options: &RenderOptions,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        Self::new_pool(api, bank, source, routes, options, cancel, routes.len())
    }
    pub(crate) fn new_pool(
        api: &'a Api,
        bank: &mut Bank,
        source: &str,
        routes: &[RouteAssignment],
        options: &RenderOptions,
        cancel: &AtomicBool,
        groups: usize,
    ) -> Result<Self> {
        if groups < routes.len() {
            return Err(AudioError::InvalidPlan(
                "Pool cannot omit verified source palette initialization channels".into(),
            ));
        }
        let mix = options.local_midi_mix.as_ref().ok_or_else(|| {
            AudioError::InvalidPlan("Dry groups require a source-bound mixer".into())
        })?;
        mix.verify_source(source)?;
        mix.verify_routes(routes)?;
        let proof = options.bank_verification.as_ref().ok_or_else(|| {
            AudioError::InvalidPlan("Dry groups require exact bank and palette verification".into())
        })?;
        if proof.sha256 != bank.hash || routes.is_empty() {
            return Err(AudioError::InvalidPlan(
                "Dry group bank identity or route count mismatch".into(),
            ));
        }
        let functions = DryApi::load(api)?;
        let synth = Synth::new_configured(api, bank, routes, options, cancel, Some(groups))?;
        if synth.names != proof.preset_names {
            return Err(AudioError::Native("Dry group palette mismatch".into()));
        }
        // SAFETY: live synth/settings, same offline thread, valid ABI pointers.
        unsafe {
            if (functions.groups)(synth.pointer()) != groups as i32
                || (functions.channels)(synth.pointer()) != groups as i32
            {
                return Err(AudioError::Native(
                    "Dry output group/channel readback mismatch".into(),
                ));
            }
            for name in [c"synth.reverb.active", c"synth.chorus.active"] {
                let mut enabled = -1;
                check(
                    (functions.settings_int)(synth.settings.as_ptr(), name.as_ptr(), &mut enabled),
                    "read FX setting",
                )?;
                if enabled != 0 {
                    return Err(AudioError::Native(
                        "Dry groups require disabled effects".into(),
                    ));
                }
            }
        }
        let mut authorized_bindings = Vec::new();
        for channel in 0..routes.len() {
            authorized_bindings.push(Self::read_binding(&synth, channel, channel)?);
        }
        Ok(Self {
            synth,
            functions,
            authorized_bindings,
            lane_bindings: (0..groups)
                .map(|n| if n < routes.len() { n } else { usize::MAX })
                .collect(),
            groups,
            frame: 0,
            source_sha256: mix.source_sha256().into(),
            owners: vec![None; groups],
        })
    }
    fn read_binding(synth: &Synth<'_>, channel: usize, route_index: usize) -> Result<RouteBinding> {
        let (mut bank_id, mut bank, mut program, mut cc7, mut cc10) = (0, 0, 0, 0, 0);
        // SAFETY: validated initialized channel, live same-thread synth, valid outputs.
        unsafe {
            check(
                (synth.api.get_program)(
                    synth.pointer(),
                    channel as i32,
                    &mut bank_id,
                    &mut bank,
                    &mut program,
                ),
                "read dry route preset",
            )?;
            check(
                (synth.api.get_cc)(synth.pointer(), channel as i32, 7, &mut cc7),
                "read dry route volume",
            )?;
            check(
                (synth.api.get_cc)(synth.pointer(), channel as i32, 10, &mut cc10),
                "read dry route pan",
            )?;
        }
        Ok(RouteBinding {
            route_index,
            bank_id,
            bank,
            program,
            cc7,
            cc10,
        })
    }
    /// Rebind only to an already source-authorized, bank-verified initial route.
    /// No caller-supplied preset/controller or fabricated mixer is accepted.
    /// API supports only NoteOn/Off and these two CCs, so all other channel state
    /// remains the untouched melodic initialization. Do not extend this premise
    /// to sustain, pressure, bend, tuning, or arbitrary MIDI without a new proof.
    pub(crate) fn reselect_idle_route(
        &mut self,
        lane: usize,
        route_index: usize,
    ) -> Result<RouteBinding> {
        let wanted = self
            .authorized_bindings
            .get(route_index)
            .cloned()
            .ok_or_else(|| {
                AudioError::InvalidPlan("Unrecognized source-authorized route".into())
            })?;
        let owner = self
            .owners
            .get(lane)
            .ok_or_else(|| AudioError::InvalidPlan("Unrecognized dry lane".into()))?;
        if owner.as_ref().is_some_and(|(_, _, released)| !released)
            || self.snapshot()?.playing_per_channel[lane] != 0
        {
            return Err(AudioError::Unsupported(
                "Cannot reselect a dry route before NoteOff and complete native tail release"
                    .into(),
            ));
        }
        // SAFETY: lane is admitted and has no playing voices; wanted values came
        // from exact bank/preset/CC readback at source-bound initialization.
        unsafe {
            check(
                (self.synth.api.channel_type)(self.synth.pointer(), lane as i32, 0),
                "initialize admitted melodic dry channel",
            )?;
            check(
                (self.synth.api.select)(
                    self.synth.pointer(),
                    lane as i32,
                    wanted.bank_id,
                    wanted.bank,
                    wanted.program,
                ),
                "reselect authorized idle preset",
            )?;
            for (cc, value) in [(7, wanted.cc7), (10, wanted.cc10)] {
                check(
                    (self.synth.api.cc)(self.synth.pointer(), lane as i32, cc, value),
                    "restore authorized route controller",
                )?;
            }
        }
        let actual = Self::read_binding(&self.synth, lane, route_index)?;
        if actual != wanted {
            return Err(AudioError::Native(
                "Rebound route preset/controller readback mismatch".into(),
            ));
        }
        self.lane_bindings[lane] = route_index;
        Ok(actual)
    }
    /// Actual installed backend settings ranges, not a score-size product cap.
    pub(crate) fn physical_group_limit(api: &Api) -> Result<usize> {
        type Range = unsafe extern "C" fn(Ptr, *const c_char, *mut c_int, *mut c_int) -> c_int;
        let function: Range =
            unsafe { std::mem::transmute(api._library.symbol(c"fluid_settings_getint_range")?) };
        let settings = NonNull::new(unsafe { (api.new_settings)() })
            .ok_or_else(|| AudioError::Native("No group-range settings".into()))?;
        let mut limit = i32::MAX;
        let mut failure = None;
        for name in [
            c"synth.audio-groups",
            c"synth.audio-channels",
            c"synth.midi-channels",
        ] {
            let (mut min, mut max) = (0, 0);
            let status = unsafe { function(settings.as_ptr(), name.as_ptr(), &mut min, &mut max) };
            if status != 0 || min < 0 || max <= 0 {
                failure = Some(AudioError::Native(
                    "Invalid native output-group range".into(),
                ));
                break;
            }
            limit = limit.min(max);
        }
        unsafe { (api.delete_settings)(settings.as_ptr()) };
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(limit as usize)
    }
    pub(crate) fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub(crate) fn quantum(&self) -> u32 {
        self.synth.quantum
    }
    pub(crate) fn snapshot(&self) -> Result<VoiceSnapshot> {
        // SAFETY: no rendering or API mutation occurs while inspecting pointers.
        // Full configured polyphony plus null sentinel, not a capped voice list.
        let capacity = unsafe { (self.functions.polyphony)(self.synth.pointer()) };
        if capacity <= 0 || capacity == i32::MAX {
            return Err(AudioError::Native("Invalid native polyphony".into()));
        }
        let mut voices = vec![std::ptr::null_mut(); capacity as usize + 1];
        unsafe {
            (self.functions.list)(self.synth.pointer(), voices.as_mut_ptr(), capacity + 1, -1)
        };
        if !voices[capacity as usize].is_null() {
            return Err(AudioError::Native(
                "Native voice list exceeded capacity".into(),
            ));
        }
        let mut counts = vec![0; self.groups];
        for voice in voices.into_iter().take_while(|v| !v.is_null()) {
            if unsafe { (self.functions.playing)(voice) } == 0 {
                continue;
            }
            let channel = unsafe { (self.functions.channel)(voice) };
            let count = usize::try_from(channel)
                .ok()
                .and_then(|ch| counts.get_mut(ch))
                .ok_or_else(|| {
                    AudioError::Native("Playing voice on unadmitted/aliased dry channel".into())
                })?;
            *count += 1;
        }
        if counts.iter().sum::<usize>() >= capacity as usize {
            return Err(AudioError::PolyphonyLimit {
                voices: capacity as u32,
                limit: capacity as u32,
            });
        }
        Ok(VoiceSnapshot {
            frame: self.frame,
            playing_per_channel: counts,
        })
    }
    pub(crate) fn event(&mut self, note: &crate::plan::Note, on: bool) -> Result<bool> {
        let channel = usize::from(note.channel);
        if channel >= self.groups || note.key > 127 || note.velocity == 0 || note.velocity > 127 {
            return Err(AudioError::InvalidPlan(
                "Invalid dry note or modulo-aliased channel".into(),
            ));
        }
        if on {
            if self.lane_bindings[channel] == usize::MAX {
                return Err(AudioError::InvalidPlan(
                    "Dry lane has no authorized preset binding".into(),
                ));
            }
            if self.owners[channel]
                .as_ref()
                .is_some_and(|(_, _, released)| !released)
                || self.snapshot()?.playing_per_channel[channel] != 0
            {
                return Err(AudioError::Unsupported(
                    "Dry channel still owns playing voices, including release tails".into(),
                ));
            }
            self.synth.event(note, true)?;
            self.owners[channel] = Some((note.event_id.clone(), note.key, false));
            self.snapshot()?; // fail on exhausted native voice budget, never silently continue
            Ok(false)
        } else {
            if self.owners[channel].as_ref() != Some(&(note.event_id.clone(), note.key, false)) {
                return Err(AudioError::InvalidPlan("Dry NoteOff owner mismatch".into()));
            }
            let missing = self.synth.event(note, false)?;
            self.owners[channel] = Some((note.event_id.clone(), note.key, true));
            Ok(missing)
        }
    }
    /// Returns planar L0,R0,L1,R1... with distinct zeroed buffers. Quantum-aligned
    /// lengths keep public voice snapshots and completed PCM on the same clock.
    pub(crate) fn render(&mut self, frames: u32) -> Result<Vec<Vec<f32>>> {
        if frames == 0 || frames % self.quantum() != 0 || frames > i32::MAX as u32 {
            return Err(AudioError::InvalidPlan(
                "Dry block must contain whole native quanta".into(),
            ));
        }
        let end = self
            .frame
            .checked_add(u64::from(frames))
            .ok_or_else(|| AudioError::InvalidPlan("Dry frame overflow".into()))?;
        let mut output = vec![vec![0.0; frames as usize]; self.groups * 2];
        let mut pointers: Vec<_> = output.iter_mut().map(|v| v.as_mut_ptr()).collect();
        // SAFETY: all 2*groups distinct zeroed buffers contain frames writable
        // floats; no FX buffers, exact output count prevents modulo mixing.
        check(
            unsafe {
                (self.functions.process)(
                    self.synth.pointer(),
                    frames as i32,
                    0,
                    std::ptr::null_mut(),
                    pointers.len() as i32,
                    pointers.as_mut_ptr(),
                )
            },
            "render disjoint dry PCM",
        )?;
        self.frame = end;
        self.snapshot()?;
        if output.iter().flatten().any(|x| !x.is_finite()) {
            return Err(AudioError::Native("Nonfinite dry PCM".into()));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests;

impl crate::dry_lanes::LaneVoiceState for DrySynth<'_> {
    fn completed_frame(&self) -> u64 {
        self.frame
    }
    fn playing_voices(&self, lane: usize) -> Result<usize> {
        self.snapshot()?
            .playing_per_channel()
            .get(lane)
            .copied()
            .ok_or_else(|| AudioError::InvalidPlan("Unknown dry voice lane".into()))
    }
}
