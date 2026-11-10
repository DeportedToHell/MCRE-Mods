//! PlayerAnimationLibrary's compiled animation (`Animation`, `BoneAnimation`, `KeyframeStack`,
//! `Keyframe`, `ExtraAnimationData`, `Animation.LoopType`) and what Emotecraft keeps beside it
//! (its header, icon, song).
//!
//! Ported from PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT; files based on
//! GeckoLib, Copyright (c) 2024 GeckoLib, MIT), see NOTICE.

use crate::easing::Easing;
use crate::molang::Exprs;
use crate::opus::OpusSound;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One keyframe: its length in ticks, its start and end values, its easing.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub length: f32,
    pub start: Exprs,
    pub end: Exprs,
    pub easing: Easing,
    pub args: Vec<Exprs>,
}

impl Keyframe {
    pub fn new(length: f32, start: Exprs, end: Exprs, easing: Easing, args: Vec<Exprs>) -> Keyframe {
        Keyframe { length, start, end, easing, args }
    }
}

/// `Keyframe.getLastKeyframeTime`.
pub fn last_time(list: &[Keyframe]) -> f32 {
    list.iter().map(|k| f64::from(k.length)).sum::<f64>() as f32
}

/// `KeyframeStack`: x, y, z.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub x: Vec<Keyframe>,
    pub y: Vec<Keyframe>,
    pub z: Vec<Keyframe>,
}

impl Stack {
    pub fn axis(&self, i: usize) -> &Vec<Keyframe> {
        match i {
            0 => &self.x,
            1 => &self.y,
            _ => &self.z,
        }
    }
    pub fn axis_mut(&mut self, i: usize) -> &mut Vec<Keyframe> {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => &mut self.z,
        }
    }
    pub fn last_time(&self) -> f32 {
        last_time(&self.x).max(last_time(&self.y).max(last_time(&self.z)))
    }
    pub fn has_keyframes(&self) -> bool {
        !self.x.is_empty() || !self.y.is_empty() || !self.z.is_empty()
    }
}

/// `BoneAnimation`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneAnim {
    pub rotation: Stack,
    pub position: Stack,
    pub scale: Stack,
    pub bend: Vec<Keyframe>,
}

impl BoneAnim {
    pub fn has_keyframes(&self) -> bool {
        self.rotation.has_keyframes() || self.position.has_keyframes() || self.scale.has_keyframes() || !self.bend.is_empty()
    }
}

/// `Animation.LoopType`: what happens when the animation reaches its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopType {
    PlayOnce,
    Loop,
    HoldOnLastFrame,
    /// `returnToTickLoop(tick)`.
    ReturnTo(f32),
}

impl LoopType {
    pub fn plays_again(self) -> bool {
        !matches!(self, LoopType::PlayOnce)
    }
    pub fn restart_tick(self) -> f32 {
        match self {
            LoopType::ReturnTo(t) => t,
            _ => 0.0,
        }
    }
    /// `LoopType.fromString` of the registered names.
    pub fn from_name(s: &str) -> LoopType {
        match s {
            "loop" | "true" => LoopType::Loop,
            "hold_on_last_frame" => LoopType::HoldOnLastFrame,
            _ => LoopType::PlayOnce,
        }
    }
}

/// `AnimationFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    GeckoLib,
    PlayerAnimator,
}

/// An `ExtraAnimationData` value as JSON leaves it (`getValue`): a boolean, a string (objects
/// and nulls as their JSON text), a float, a list.
#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    Bool(bool),
    Float(f32),
    Str(String),
    List(Vec<Extra>),
}

impl Extra {
    pub fn from_json(v: &serde_json::Value) -> Extra {
        match v {
            serde_json::Value::Bool(b) => Extra::Bool(*b),
            serde_json::Value::String(s) => Extra::Str(s.clone()),
            serde_json::Value::Number(n) => Extra::Float(n.as_f64().unwrap_or(0.0) as f32),
            serde_json::Value::Array(a) => Extra::List(a.iter().map(Extra::from_json).collect()),
            other => Extra::Str(other.to_string()),
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Extra::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Extra::Float(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Extra::Bool(b) => Some(*b),
            Extra::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
}

/// An event keyframe (`SoundKeyframeData`, `ParticleKeyframeData`, `CustomInstructionKeyframeData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Events {
    pub sounds: Vec<(f32, String)>,
    pub particles: Vec<(f32, String, String, String)>,
    pub instructions: Vec<(f32, String)>,
}

/// A compiled animation (`Animation`) and its data.
#[derive(Clone, Debug)]
pub struct Animation {
    pub length: f32,
    pub loop_type: LoopType,
    /// Bone name → its keyframes; ordered by name, as PlayerAnimator's loader keeps them.
    pub bones: BTreeMap<String, BoneAnim>,
    pub events: Events,
    /// Custom pivot bones (`model`), in pixels.
    pub pivots: BTreeMap<String, [f32; 3]>,
    /// Bone → parent bone (`parents`).
    pub parents: BTreeMap<String, String>,
    pub format: Format,
    pub begin_tick: Option<f32>,
    pub end_tick: Option<f32>,
    /// `easeBeforeKeyframe`, true when absent.
    pub ease_before: Option<bool>,
    pub apply_bend: bool,
    /// `disableAxisIfNotModified`, true when absent.
    pub disable_axis: bool,
    /// The rest of `ExtraAnimationData`: `name`, `description`, `author` (JSON text components
    /// or plain strings), `folderpath`, `fileName`, `bages`, `isBuiltin`, `previewTick`...
    pub extra: BTreeMap<String, Extra>,
    pub uuid: u128,
    /// The icon's PNG bytes (`iconData`).
    pub icon: Option<Arc<Vec<u8>>>,
    /// The song (`SongPacket.OPUS_KEY`).
    pub sound: Option<Arc<OpusSound>>,
    /// An `.nbs` song, only passed on (`SongPacket.NBS_KEY`).
    pub nbs: Option<Arc<Vec<u8>>>,
}

impl PartialEq for Animation {
    /// `Animation.equals`: the keyframes and model, not the data.
    fn eq(&self, o: &Animation) -> bool {
        self.length == o.length && self.bones == o.bones && self.events == o.events && self.pivots == o.pivots && self.parents == o.parents
    }
}

impl Animation {
    pub fn new(length: f32, loop_type: LoopType) -> Animation {
        Animation {
            length,
            loop_type,
            bones: BTreeMap::new(),
            events: Events::default(),
            pivots: BTreeMap::new(),
            parents: BTreeMap::new(),
            format: Format::GeckoLib,
            begin_tick: None,
            end_tick: None,
            ease_before: None,
            apply_bend: false,
            disable_axis: true,
            extra: BTreeMap::new(),
            uuid: 0,
            icon: None,
            sound: None,
            nbs: None,
        }
    }

    /// `isPlayingAt(tick)`.
    pub fn is_playing_at(&self, tick: f32) -> bool {
        self.loop_type.plays_again() || tick < self.length && tick > 0.0
    }

    pub fn is_easing_before(&self) -> bool {
        self.ease_before.unwrap_or(true)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(Extra::as_str)
    }

    /// The `uuid` key if the data has one, else one made from the keyframes (`generateUuid`:
    /// Java hashes its own objects; this hashes the animation's binary form instead, so the
    /// same file gives the same id every time).
    pub fn settle_uuid(&mut self) {
        if let Some(u) = self.str("uuid").and_then(crate::buf::parse_uuid) {
            self.uuid = u;
            return;
        }
        if self.uuid != 0 {
            return;
        }
        let mut w = crate::buf::Writer::new();
        crate::binary::write_body(&mut w, self);
        let mut a: u64 = 17;
        let mut b: u64 = 31;
        for chunk in w.data.chunks(4) {
            let mut v = [0u8; 4];
            v[..chunk.len()].copy_from_slice(chunk);
            let h = u64::from(u32::from_be_bytes(v));
            a = a.wrapping_mul(31).wrapping_add(h);
            b = b.wrapping_mul(37).wrapping_add(h);
        }
        a = a.wrapping_mul(31).wrapping_add(u64::from(self.length.to_bits()));
        self.uuid = (u128::from(a) << 64) | u128::from(b);
    }

    /// `getNameOrId` (a name, lower case, quotes dropped, spaces as underscores).
    pub fn name_or_id(&self) -> String {
        match self.str("name") {
            Some(n) => n.to_lowercase().replace('"', "").replace(' ', "_"),
            None => crate::buf::uuid_string(self.uuid),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.extra.contains_key("isBuiltin")
    }

    /// `PlayerChooseElement.calculatePreviewTick`.
    pub fn preview_tick(&self) -> f32 {
        if let Some(t) = self.extra.get("previewTick").and_then(Extra::as_f32) {
            return t;
        }
        if self.loop_type.plays_again() {
            let r = self.loop_type.restart_tick();
            if r > 0.0 {
                return (self.length - r) / 2.0;
            }
        }
        self.length / 2.0
    }
}
