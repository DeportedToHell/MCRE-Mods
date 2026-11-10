//! PlayerAnimationLibrary's `AnimationBinary` (versions 1 to 5) and `AnimationBinaryV6` (the
//! bit-packed header, the axes' presence flags, compact keyframes), as the `NewAnimPacket`
//! (0x99) carries them.
//!
//! Ported from PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT), see NOTICE.

use crate::anim::{Animation, BoneAnim, Events, Format, Keyframe, LoopType, Stack};
use crate::buf::{Reader, Result, Writer, err};
use crate::easing::Easing;
use crate::molang::{self, Exprs};
use std::collections::BTreeMap;

pub const CURRENT_VERSION: u8 = 6;

const SHOULD_PLAY_AGAIN: i32 = 1;
const HOLD_ON_LAST_FRAME: i32 = 2;
const PLAYER_ANIMATOR: i32 = 4;
const APPLY_BEND: i32 = 8;
const EASE_BEFORE: i32 = 16;
const HAS_BEGIN_TICK: i32 = 32;
const HAS_END_TICK: i32 = 64;

const IS_CONSTANT: i32 = 1;
const HAS_EASING_ARGS: i32 = 2;
const LENGTH_ZERO: i32 = 4;
const LENGTH_ONE: i32 = 8;
const FLAG_BITS: u32 = 4;

fn is_item(name: &str) -> bool {
    name == "right_item" || name == "left_item"
}

fn default_start(scale: bool) -> Exprs {
    molang::constant(if scale { 1.0 } else { 0.0 })
}

// ---- version 6 ----------------------------------------------------------------------------

fn channels(b: &BoneAnim) -> [&Vec<Keyframe>; 10] {
    [&b.rotation.x, &b.rotation.y, &b.rotation.z, &b.position.x, &b.position.y, &b.position.z, &b.scale.x, &b.scale.y, &b.scale.z, &b.bend]
}

fn write_keyframe_v6(w: &mut Writer, k: &Keyframe, rt: &mut molang::Runtime) {
    let constant = molang::single_constant(&k.end);
    let has_args = k.args.iter().any(|a| !a.is_empty());
    let mut flags = 0;
    if constant {
        flags |= IS_CONSTANT;
    }
    if has_args {
        flags |= HAS_EASING_ARGS;
    }
    if k.length == 0.0 {
        flags |= LENGTH_ZERO;
    } else if k.length == 1.0 {
        flags |= LENGTH_ONE;
    }
    w.varint((i32::from(k.easing.id()) << FLAG_BITS) | flags);
    if constant {
        w.f32(rt.eval(&k.end));
    } else {
        molang::write_exprs(&k.end, w);
    }
    if flags & (LENGTH_ZERO | LENGTH_ONE) == 0 {
        w.f32(k.length);
    }
    if has_args {
        w.varint(k.args.len() as i32);
        for a in &k.args {
            molang::write_exprs(a, w);
        }
    }
}

fn read_list_v6(r: &mut Reader<'_>, from_default: bool, scale: bool) -> Result<Vec<Keyframe>> {
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad keyframe count");
    }
    let mut list: Vec<Keyframe> = Vec::with_capacity(n as usize);
    for i in 0..n {
        let combined = r.varint()?;
        let easing = Easing::from_id(((combined as u32) >> FLAG_BITS) as u8);
        let flags = combined & ((1 << FLAG_BITS) - 1);
        let end = if flags & IS_CONSTANT != 0 { molang::constant(r.f32()?) } else { molang::read_exprs(r)? };
        let length = if flags & LENGTH_ZERO != 0 {
            0.0
        } else if flags & LENGTH_ONE != 0 {
            1.0
        } else {
            r.f32()?
        };
        let start = match list.last() {
            Some(k) => k.end.clone(),
            None if from_default => default_start(scale),
            None => end.clone(),
        };
        let args = if flags & HAS_EASING_ARGS != 0 {
            read_args(r)?
        } else if from_default && i > 0 {
            crate::loader::empty_easing_arg()
        } else {
            Vec::new()
        };
        list.push(Keyframe::new(length, start, end, easing, args));
    }
    Ok(list)
}

fn read_args(r: &mut Reader<'_>) -> Result<Vec<Exprs>> {
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad easing argument count");
    }
    (0..n).map(|_| molang::read_exprs(r)).collect()
}

/// The keyframes, events and model: everything after the header (also what an emote's own id
/// is made from when it has none).
pub fn write_body(w: &mut Writer, a: &Animation) {
    let mut rt = molang::Runtime::new();
    w.varint(a.bones.len() as i32);
    for (name, b) in &a.bones {
        w.vstring(Some(name));
        let ch = channels(b);
        let mut presence = 0;
        for (i, c) in ch.iter().enumerate() {
            if !c.is_empty() {
                presence |= 1 << i;
            }
        }
        w.varint(presence);
        for c in ch {
            if !c.is_empty() {
                w.varint(c.len() as i32);
                for k in c {
                    write_keyframe_v6(w, k, &mut rt);
                }
            }
        }
    }
    write_tail(w, a);
}

fn write_tail(w: &mut Writer, a: &Animation) {
    write_events(w, &a.events);
    w.varint(a.pivots.len() as i32);
    for (k, v) in &a.pivots {
        w.vstring(Some(k));
        for c in v {
            w.f32(*c);
        }
    }
    w.varint(a.parents.len() as i32);
    for (k, v) in &a.parents {
        w.vstring(Some(k));
        w.vstring(Some(v));
    }
}

fn write_events(w: &mut Writer, e: &Events) {
    w.varint(e.sounds.len() as i32);
    for (t, s) in &e.sounds {
        w.f32(*t);
        w.vstring(Some(s));
    }
    w.varint(e.particles.len() as i32);
    for (t, a, b, c) in &e.particles {
        w.f32(*t);
        w.vstring(Some(a));
        w.vstring(Some(b));
        w.vstring(Some(c));
    }
    w.varint(e.instructions.len() as i32);
    for (t, s) in &e.instructions {
        w.f32(*t);
        w.vstring(Some(s));
    }
}

fn read_events(r: &mut Reader<'_>) -> Result<Events> {
    let mut e = Events::default();
    let count = |r: &mut Reader<'_>| -> Result<usize> {
        let n = r.varint()?;
        if n < 0 || n as usize > r.remaining() {
            return err("bad event count");
        }
        Ok(n as usize)
    };
    for _ in 0..count(r)? {
        let t = r.f32()?;
        e.sounds.push((t, r.vstring()?.unwrap_or_default()));
    }
    for _ in 0..count(r)? {
        let t = r.f32()?;
        let a = r.vstring()?.unwrap_or_default();
        let b = r.vstring()?.unwrap_or_default();
        let c = r.vstring()?.unwrap_or_default();
        e.particles.push((t, a, b, c));
    }
    for _ in 0..count(r)? {
        let t = r.f32()?;
        e.instructions.push((t, r.vstring()?.unwrap_or_default()));
    }
    Ok(e)
}

fn read_tail(r: &mut Reader<'_>, a: &mut Animation) -> Result<()> {
    a.events = read_events(r)?;
    let n = r.varint()?;
    for _ in 0..n.max(0) {
        let k = r.vstring()?.unwrap_or_default();
        a.pivots.insert(k, [r.f32()?, r.f32()?, r.f32()?]);
    }
    let n = r.varint()?;
    for _ in 0..n.max(0) {
        let k = r.vstring()?.unwrap_or_default();
        a.parents.insert(k, r.vstring()?.unwrap_or_default());
    }
    Ok(())
}

fn write_v6(w: &mut Writer, a: &Animation) {
    let again = a.loop_type.plays_again();
    let hold = a.loop_type == LoopType::HoldOnLastFrame;
    let mut flags = 0;
    if again {
        flags |= SHOULD_PLAY_AGAIN;
    }
    if hold {
        flags |= HOLD_ON_LAST_FRAME;
    }
    if a.format == Format::PlayerAnimator {
        flags |= PLAYER_ANIMATOR;
    }
    if a.apply_bend {
        flags |= APPLY_BEND;
    }
    if a.is_easing_before() {
        flags |= EASE_BEFORE;
    }
    if a.begin_tick.is_some() {
        flags |= HAS_BEGIN_TICK;
    }
    if a.end_tick.is_some() {
        flags |= HAS_END_TICK;
    }
    w.varint(flags);
    w.f32(a.length);
    if again && !hold {
        w.f32(a.loop_type.restart_tick());
    }
    if let Some(t) = a.begin_tick {
        w.f32(t);
    }
    if let Some(t) = a.end_tick {
        w.f32(t);
    }
    w.uuid(a.uuid);
    write_body(w, a);
}

fn read_v6(r: &mut Reader<'_>) -> Result<Animation> {
    let flags = r.varint()?;
    let format = if flags & PLAYER_ANIMATOR != 0 { Format::PlayerAnimator } else { Format::GeckoLib };
    let length = r.f32()?;
    let loop_type = if flags & SHOULD_PLAY_AGAIN != 0 {
        if flags & HOLD_ON_LAST_FRAME != 0 { LoopType::HoldOnLastFrame } else { LoopType::ReturnTo(r.f32()?) }
    } else {
        LoopType::PlayOnce
    };
    let mut a = Animation::new(length, loop_type);
    a.format = format;
    a.apply_bend = flags & APPLY_BEND != 0;
    if flags & EASE_BEFORE == 0 {
        a.ease_before = Some(false);
    }
    if flags & HAS_BEGIN_TICK != 0 {
        a.begin_tick = Some(r.f32()?);
    }
    if flags & HAS_END_TICK != 0 {
        a.end_tick = Some(r.f32()?);
    }
    a.uuid = r.uuid()?;
    let from_default = format == Format::PlayerAnimator;
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad bone count");
    }
    for _ in 0..n {
        let name = r.vstring()?.unwrap_or_default();
        let presence = r.varint()?;
        let mut lists: Vec<Vec<Keyframe>> = Vec::with_capacity(10);
        for i in 0..10 {
            lists.push(if presence & (1 << i) != 0 { read_list_v6(r, from_default, (6..9).contains(&i))? } else { Vec::new() });
        }
        let mut it = lists.into_iter();
        let mut next = || it.next().unwrap_or_default();
        let rotation = Stack { x: next(), y: next(), z: next() };
        let position = Stack { x: next(), y: next(), z: next() };
        let scale = Stack { x: next(), y: next(), z: next() };
        let bend = next();
        a.bones.insert(name, BoneAnim { rotation, position, scale, bend });
    }
    read_tail(r, &mut a)?;
    Ok(a)
}

// ---- versions 1 to 5 ----------------------------------------------------------------------

/// `negateKeyframeExpressions`: a single constant is negated, anything else stays.
fn negate(list: &[Keyframe]) -> Vec<Keyframe> {
    let mut rt = molang::Runtime::new();
    list.iter()
        .map(|k| {
            let mut k = k.clone();
            for v in [&mut k.start, &mut k.end] {
                if molang::single_constant(v) {
                    *v = molang::constant(-rt.eval(v));
                }
            }
            k
        })
        .collect()
}

fn write_list_old(w: &mut Writer, list: &[Keyframe]) {
    w.varint(list.len() as i32);
    for k in list {
        w.f32(k.length);
        molang::write_exprs(&k.end, w);
        w.u8(k.easing.id());
        w.varint(k.args.len() as i32);
        for a in &k.args {
            molang::write_exprs(a, w);
        }
    }
}

fn read_list_old(r: &mut Reader<'_>, from_default: bool, scale: bool) -> Result<Vec<Keyframe>> {
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad keyframe count");
    }
    let mut list: Vec<Keyframe> = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let length = r.f32()?;
        let end = molang::read_exprs(r)?;
        let start = match list.last() {
            Some(k) => k.end.clone(),
            None if from_default => default_start(scale),
            None => end.clone(),
        };
        let easing = Easing::from_id(r.u8()?);
        let args = read_args(r)?;
        list.push(Keyframe::new(length, start, end, easing, args));
    }
    Ok(list)
}

fn write_old(w: &mut Writer, version: u8, a: &Animation) {
    let mut apply_bend = a.apply_bend;
    if version < 3 && apply_bend && a.bones.get("torso").is_some_and(|t| !t.bend.is_empty()) {
        apply_bend = false;
    }
    w.f32(a.length);
    let again = a.loop_type.plays_again();
    w.bool(again);
    if again {
        if a.loop_type == LoopType::HoldOnLastFrame {
            w.bool(true);
        } else {
            w.bool(false);
            w.f32(a.loop_type.restart_tick());
        }
    }
    w.u8(if a.format == Format::PlayerAnimator { 1 } else { 0 });
    w.f32(a.begin_tick.unwrap_or(f32::NAN));
    w.f32(a.end_tick.unwrap_or(f32::NAN));
    if version > 1 {
        w.bool(apply_bend);
        w.bool(a.is_easing_before());
    }
    w.uuid(a.uuid);
    w.varint(a.bones.len() as i32);
    for (name, b) in &a.bones {
        w.vstring(Some(name));
        let body = version < 4 && name == "body";
        let item = version < 5 && is_item(name);
        let maybe = |list: &Vec<Keyframe>, neg: bool| if neg { negate(list) } else { list.clone() };
        write_list_old(w, &maybe(&b.rotation.x, body));
        write_list_old(w, &maybe(&b.rotation.y, body));
        write_list_old(w, &b.rotation.z);
        write_list_old(w, &maybe(&b.position.x, body));
        write_list_old(w, &maybe(&b.position.y, item));
        write_list_old(w, &b.position.z);
        write_list_old(w, &b.scale.x);
        write_list_old(w, &b.scale.y);
        write_list_old(w, &b.scale.z);
        write_list_old(w, &b.bend);
    }
    write_tail(w, a);
}

fn read_old(r: &mut Reader<'_>, version: u8) -> Result<Animation> {
    let length = r.f32()?;
    let loop_type = if r.bool()? {
        if r.bool()? { LoopType::HoldOnLastFrame } else { LoopType::ReturnTo(r.f32()?) }
    } else {
        LoopType::PlayOnce
    };
    let mut a = Animation::new(length, loop_type);
    a.format = if r.i8()? == 0 { Format::GeckoLib } else { Format::PlayerAnimator };
    let begin = r.f32()?;
    let end = r.f32()?;
    if !begin.is_nan() {
        a.begin_tick = Some(begin);
    }
    if !end.is_nan() {
        a.end_tick = Some(end);
    }
    if version > 1 {
        a.apply_bend = r.bool()?;
        if !r.bool()? {
            a.ease_before = Some(false);
        }
    } else {
        a.apply_bend = true;
    }
    a.uuid = r.uuid()?;
    let from_default = a.format == Format::PlayerAnimator;
    let n = r.varint()?;
    if n < 0 || n as usize > r.remaining() {
        return err("bad bone count");
    }
    let mut bones = BTreeMap::new();
    for _ in 0..n {
        let name = r.vstring()?.unwrap_or_default();
        let rotation = Stack { x: read_list_old(r, from_default, false)?, y: read_list_old(r, from_default, false)?, z: read_list_old(r, from_default, false)? };
        let position = Stack { x: read_list_old(r, from_default, false)?, y: read_list_old(r, from_default, false)?, z: read_list_old(r, from_default, false)? };
        let scale = Stack { x: read_list_old(r, from_default, true)?, y: read_list_old(r, from_default, true)?, z: read_list_old(r, from_default, true)? };
        let bend = read_list_old(r, from_default, false)?;
        bones.insert(name, BoneAnim { rotation, position, scale, bend });
    }
    if version < 4
        && let Some(body) = bones.get_mut("body")
    {
        body.position.x = negate(&body.position.x);
        body.rotation.x = negate(&body.rotation.x);
        body.rotation.y = negate(&body.rotation.y);
    }
    if version < 5 {
        for item in ["right_item", "left_item"] {
            if let Some(b) = bones.get_mut(item) {
                b.position.y = negate(&b.position.y);
            }
        }
    }
    a.bones = bones;
    read_tail(r, &mut a)?;
    Ok(a)
}

/// `AnimationBinary.write(buf, version, animation)`.
pub fn write(w: &mut Writer, version: u8, a: &Animation) {
    if version >= 6 {
        write_v6(w, a);
    } else {
        write_old(w, version, a);
    }
}

/// `AnimationBinary.read(buf, version)`.
pub fn read(r: &mut Reader<'_>, version: u8) -> Result<Animation> {
    if version >= 6 { read_v6(r) } else { read_old(r, version) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_round_trips_in_every_version() {
        for text in crate::tests::BUILT_IN_JSON {
            let a = &crate::loader::load_json(text).unwrap()[0];
            for v in 1..=6 {
                let mut w = Writer::new();
                write(&mut w, v, a);
                let b = read(&mut Reader::new(&w.data), v).unwrap();
                // A first keyframe's empty easing argument isn't sent (as in Java): compare what
                // the decoded animation writes again.
                let mut w2 = Writer::new();
                write(&mut w2, v, &b);
                let first = w2.data.iter().zip(&w.data).position(|(x, y)| x != y);
                assert!(first.is_none() && w2.data.len() == w.data.len(), "version {v}: {} vs {} bytes, first difference at {first:?}: {:?} vs {:?}", w2.len(), w.len(), first.map(|i| &w2.data[i.saturating_sub(8)..(i + 8).min(w2.len())]), first.map(|i| &w.data[i.saturating_sub(8)..(i + 8).min(w.len())]));
                assert_eq!(b.bones.len(), a.bones.len());
                assert_eq!(b.uuid, a.uuid);
                // A loop comes back as a loop to tick 0, as in Java.
                assert_eq!(b.loop_type.plays_again(), a.loop_type.plays_again());
                assert_eq!(b.loop_type.restart_tick(), a.loop_type.restart_tick());
                assert_eq!(b.begin_tick, a.begin_tick);
            }
        }
    }
}
