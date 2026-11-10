//! Reading emotes: PlayerAnimator's JSON (`PlayerAnimatorLoader`, Emotecraft's own format,
//! versions 1 to 3), Bedrock / GeckoLib / Blockbench animations (`AnimationLoader`,
//! `KeyFrameLoader`) and which of them a file is (`UniversalAnimLoader`).
//!
//! Ported from PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT; parts based on
//! GeckoLib, Copyright (c) 2024 GeckoLib, MIT, and on playerAnimator, Copyright (c) 2022 KosmX,
//! MIT), see NOTICE.

use crate::anim::{Animation, BoneAnim, Events, Extra, Format, Keyframe, LoopType, Stack, last_time};
use crate::easing::Easing;
use crate::molang::{self, Expr, Exprs};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub type LoadResult<T> = std::result::Result<T, String>;

fn zero() -> Exprs {
    molang::constant(0.0)
}
fn one() -> Exprs {
    molang::constant(1.0)
}
/// `PlayerAnimatorLoader.EMPTY_EASING_ARG`: one empty argument.
pub fn empty_easing_arg() -> Vec<Exprs> {
    vec![Vec::new()]
}

/// `UniversalAnimLoader.getCorrectPlayerBoneName`: `rightArm` → `right_arm`.
pub fn bone_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn as_f32(v: &Value) -> Option<f32> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| f as f32),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}
fn as_i32(v: &Value) -> Option<i32> {
    match v {
        Value::Number(n) => n.as_i64().map(|i| i as i32).or_else(|| n.as_f64().map(|f| f as i32)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}
/// Gson's `getAsBoolean`: a string is `Boolean.parseBoolean`.
fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// `ExtraAnimationData.fromJson`.
fn extra_from(node: &Map<String, Value>, root: bool, into: &mut BTreeMap<String, Extra>) {
    for (k, v) in node {
        if root && (k.eq_ignore_ascii_case("version") || k.eq_ignore_ascii_case("emote")) {
            continue;
        }
        into.insert(k.clone(), Extra::from_json(v));
    }
}

/// `UniversalAnimLoader.loadAnimations`: the animations of a JSON file, by name.
pub fn load_json(text: &str) -> LoadResult<Vec<Animation>> {
    let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let Value::Object(obj) = json else { return Err("not a JSON object".into()) };
    if let Some(Value::Object(anims)) = obj.get("animations") {
        let mut out = Vec::new();
        let root_parents = obj.get("parents").and_then(Value::as_object);
        let root_model = obj.get("model").and_then(Value::as_object);
        for (name, a) in anims {
            let Value::Object(a) = a else { continue };
            match bedrock(a) {
                Ok(mut anim) => {
                    if !anim.extra.contains_key("name") {
                        anim.extra.insert("name".into(), Extra::Str(name.clone()));
                    }
                    if let (Some(p), Some(m)) = (root_parents, root_model) {
                        if anim.pivots.is_empty() {
                            anim.pivots = model_of(m);
                        }
                        if anim.parents.is_empty() {
                            anim.parents = parents_of(p);
                        }
                    }
                    anim.settle_uuid();
                    out.push(anim);
                }
                Err(e) => log::error!("[Emotecraft] Unable to parse animation: {name}: {e}"),
            }
        }
        Ok(out)
    } else {
        let mut anim = legacy(&obj)?;
        anim.settle_uuid();
        Ok(vec![anim])
    }
}

// ---- PlayerAnimator's JSON ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Ty {
    Position,
    Rotation,
    Scale,
    Bend,
}

/// `PlayerAnimatorLoader.getDefaultValues`.
fn default_values(bone: &str) -> [f32; 3] {
    match bone {
        "right_arm" => [-5.0, 2.0, 0.0],
        "left_arm" => [5.0, 2.0, 0.0],
        "left_leg" => [1.9, 12.0, 0.1],
        "right_leg" => [-1.9, 12.0, 0.1],
        _ => [0.0; 3],
    }
}

fn legacy(node: &Map<String, Value>) -> LoadResult<Animation> {
    let Some(Value::Object(emote)) = node.get("emote") else {
        return Err("not an emotecraft animation".into());
    };
    let version = node.get("version").and_then(as_i32).unwrap_or(1);
    let mut anim = Animation::new(0.0, LoopType::PlayOnce);
    extra_from(node, true, &mut anim.extra);
    anim.format = Format::PlayerAnimator;
    if version > 3 {
        return Err(format!("{} is version {version}. Player Animation library can only process version 3.", anim.name_or_id()));
    }
    if version < 3 {
        anim.apply_bend = true;
    }
    let ease_before = emote.get("easeBeforeKeyframe").is_some_and(as_bool);
    anim.ease_before = Some(ease_before);
    let mut begin_tick = 0.0;
    if let Some(v) = emote.get("beginTick").and_then(as_f32) {
        begin_tick = v;
        anim.begin_tick = Some(v);
    }
    let mut end_tick = begin_tick + 1.0;
    if let Some(v) = emote.get("endTick").and_then(as_f32) {
        end_tick = v.max(end_tick);
        anim.end_tick = Some(end_tick);
    }
    if end_tick <= 0.0 {
        return Err("endTick must be bigger than 0".into());
    }
    let mut loop_type = LoopType::PlayOnce;
    if let (Some(is_loop), Some(ret)) = (emote.get("isLoop"), emote.get("returnTick")) {
        let looped = as_bool(is_loop);
        let return_tick = (as_i32(ret).unwrap_or(0) - 1).max(0);
        if looped {
            if return_tick as f32 > end_tick || return_tick < 0 {
                return Err("The returnTick has to be a non-negative value smaller than the endTick value".into());
            }
            loop_type = if return_tick == 0 { LoopType::Loop } else { LoopType::ReturnTo(return_tick as f32) };
        }
    }
    let stop_tick = emote.get("stopTick").and_then(as_f32).unwrap_or(0.0);
    if loop_type == LoopType::PlayOnce {
        end_tick = if stop_tick <= end_tick { end_tick + 3.0 } else { stop_tick };
    }
    let degrees = emote.get("degrees").is_none_or(as_bool);
    let moves = emote.get("moves").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut bones = moves_of(moves, degrees, version, end_tick);
    for (name, bone) in bones.iter_mut() {
        if !ease_before {
            for s in [&mut bone.position, &mut bone.rotation, &mut bone.scale] {
                for i in 0..3 {
                    correct_easings(s.axis_mut(i));
                }
            }
            correct_easings(&mut bone.bend);
        }
        if name == "right_item" || name == "left_item" {
            std::mem::swap(&mut bone.position.y, &mut bone.position.z);
            std::mem::swap(&mut bone.rotation.y, &mut bone.rotation.z);
        }
    }
    if let Some(Extra::Str(effect)) = anim.extra.get("particleEffects") {
        let effect = effect.clone();
        anim.events.particles.push((begin_tick, effect, "body".into(), String::new()));
    }
    anim.length = end_tick;
    anim.loop_type = loop_type;
    anim.bones = bones;
    Ok(anim)
}

/// `correctEasings`: each keyframe takes the one before's easing (the first ease-in-out-sine),
/// and a 0.001 tick keyframe at the end carries the last one's.
fn correct_easings(list: &mut Vec<Keyframe>) {
    let mut prev_easing = Easing::InOutSine;
    let mut prev_args: Vec<Exprs> = Vec::new();
    let mut last: Option<Keyframe> = None;
    for k in list.iter_mut() {
        let original = k.clone();
        k.easing = prev_easing;
        k.args = prev_args.clone();
        prev_easing = original.easing;
        prev_args = original.args.clone();
        last = Some(original);
    }
    if let Some(k) = last {
        list.push(Keyframe::new(0.001, k.end.clone(), k.end, k.easing, k.args));
    }
}

fn moves_of(mut moves: Vec<Value>, degrees: bool, version: i32, end_tick: f32) -> BTreeMap<String, BoneAnim> {
    let mut bones: BTreeMap<String, BoneAnim> = BTreeMap::new();
    moves.sort_by_key(|m| m.get("tick").and_then(as_i32).unwrap_or(0));
    for m in &moves {
        let Value::Object(obj) = m else { continue };
        let tick = obj.get("tick").and_then(as_f32).unwrap_or(0.0);
        if tick > end_tick {
            continue;
        }
        let easing = Easing::from_legacy(obj.get("easing").and_then(Value::as_str).unwrap_or("linear"));
        let turn = obj.get("turn").and_then(as_i32).unwrap_or(0);
        for (key, value) in obj {
            if matches!(key.as_str(), "tick" | "comment" | "easing" | "turn") {
                continue;
            }
            let mut name = bone_name(key);
            if version < 3 && name == "torso" {
                name = "body".into();
            }
            let Value::Object(part) = value else { continue };
            let bone = bones.entry(name.clone()).or_default();
            add_part(&name, bone, part, degrees, tick, easing, turn);
        }
    }
    if let Some(body) = bones.get_mut("body")
        && !body.bend.is_empty()
    {
        let bend = std::mem::take(&mut body.bend);
        let empty = !body.has_keyframes();
        if empty {
            bones.remove("body");
        }
        bones.entry("torso".into()).or_default().bend.extend(bend);
    }
    bones
}

fn add_part(name: &str, bone: &mut BoneAnim, part: &Map<String, Value>, degrees: bool, tick: f32, easing: Easing, turn: i32) {
    let is_item = name == "right_item" || name == "left_item";
    let is_cape = name == "cape";
    let is_body = name == "body";
    let def = default_values(name);
    fill(&mut bone.position, def, if is_body { Some(Ty::Position) } else { None }, ["x", "y", "z"], part, degrees, tick, easing, turn, is_item, is_cape, is_body);
    fill(&mut bone.rotation, [0.0; 3], Some(Ty::Rotation), ["pitch", "yaw", "roll"], part, degrees, tick, easing, turn, is_item, is_cape, is_body);
    fill(&mut bone.scale, [0.0; 3], Some(Ty::Scale), ["scaleX", "scaleY", "scaleZ"], part, degrees, tick, easing, turn, false, false, false);
    let last = last_time(&bone.bend);
    add(last, &mut bone.bend, 0.0, Some(Ty::Bend), "bend", part, degrees, tick, easing, turn, false);
}

#[allow(clippy::too_many_arguments)]
fn fill(
    stack: &mut Stack,
    def: [f32; 3],
    ty: Option<Ty>,
    keys: [&str; 3],
    part: &Map<String, Value>,
    degrees: bool,
    tick: f32,
    easing: Easing,
    turn: i32,
    is_item: bool,
    is_cape: bool,
    is_body: bool,
) {
    let lx = last_time(&stack.x);
    add(lx, &mut stack.x, def[0], ty, keys[0], part, degrees, tick, easing, turn, is_item || is_cape || is_body);
    let ly = last_time(&stack.y);
    add(ly, &mut stack.y, def[1], ty, keys[1], part, degrees, tick, easing, turn, is_item || ty.is_none() || (is_body && ty == Some(Ty::Rotation)));
    let lz = last_time(&stack.z);
    add(lz, &mut stack.z, def[2], ty, keys[2], part, degrees, tick, easing, turn, (is_item && ty == Some(Ty::Rotation)) || is_cape);
}

#[allow(clippy::too_many_arguments)]
fn add(last_tick: f32, part: &mut Vec<Keyframe>, def: f32, ty: Option<Ty>, name: &str, node: &Map<String, Value>, degrees: bool, tick: f32, easing: Easing, turn: i32, negate: bool) {
    let Some(raw) = node.get(name).and_then(as_f32) else { return };
    let prev_time = if part.is_empty() { 0.0 } else { last_tick };
    let delta = tick - prev_time;
    let mut value = raw;
    if ty.is_none() {
        value -= def;
    }
    if negate {
        value *= -1.0;
    }
    if ty == Some(Ty::Rotation) {
        if degrees {
            value *= std::f32::consts::PI / 180.0;
        }
        value += std::f32::consts::PI * 2.0 * turn as f32;
    }
    if ty == Some(Ty::Position) {
        value *= 16.0;
    }
    let start = match part.last() {
        Some(k) => k.end.clone(),
        None if ty == Some(Ty::Scale) => one(),
        None => zero(),
    };
    part.push(Keyframe::new(delta, start, molang::constant(value), easing, empty_easing_arg()));
}

// ---- Bedrock / GeckoLib -------------------------------------------------------------------

fn model_of(m: &Map<String, Value>) -> BTreeMap<String, [f32; 3]> {
    m.iter()
        .filter_map(|(k, v)| {
            let p = v.get("pivot")?.as_array()?;
            Some((k.clone(), [as_f32(p.first()?)?, as_f32(p.get(1)?)?, as_f32(p.get(2)?)?]))
        })
        .collect()
}

fn parents_of(m: &Map<String, Value>) -> BTreeMap<String, String> {
    m.iter().filter_map(|(k, v)| Some((bone_name(k), v.as_str()?.to_owned()))).collect()
}

fn bedrock(a: &Map<String, Value>) -> LoadResult<Animation> {
    let empty = Map::new();
    let bones_obj = a.get("bones").and_then(Value::as_object).unwrap_or(&empty);
    let mut bones = BTreeMap::new();
    for (name, b) in bones_obj {
        let Value::Object(b) = b else { continue };
        let scale = stack(keyframes(b.get("scale"))?, Ty::Scale)?;
        let position = stack(keyframes(b.get("position"))?, Ty::Position)?;
        let rotation = stack(keyframes(b.get("rotation"))?, Ty::Rotation)?;
        let bend = stack(keyframes(b.get("bend"))?, Ty::Bend)?;
        bones.insert(bone_name(name), BoneAnim { rotation, position, scale, bend: bend.x });
    }
    let mut length = match a.get("animation_length") {
        Some(v) => as_f32(v).ok_or("Expected animation_length to be a Float")? * 20.0,
        None => -1.0,
    };
    if length == -1.0 {
        length = length_of(&bones);
    }
    let loop_type = if let Some(t) = a.get("loopTick") {
        let r = as_f32(t).unwrap_or(0.0) * 20.0;
        if r > length || r < 0.0 {
            return Err("The returnTick has to be a non-negative value smaller than the endTick value".into());
        }
        LoopType::ReturnTo(r)
    } else {
        match a.get("loop") {
            Some(Value::Bool(true)) => LoopType::Loop,
            Some(Value::String(s)) => LoopType::from_name(s),
            _ => LoopType::PlayOnce,
        }
    };
    let mut anim = Animation::new(length, loop_type);
    anim.bones = bones;
    anim.events = events(a);
    anim.parents = a.get("parents").and_then(Value::as_object).map(parents_of).unwrap_or_default();
    anim.pivots = a.get("model").and_then(Value::as_object).map(model_of).unwrap_or_default();
    if let Some(Value::Object(data)) = a.get("player_animation_library") {
        extra_from(data, false, &mut anim.extra);
        if let Some(b) = data.get("applyBendToOtherBones") {
            anim.apply_bend = as_bool(b);
        }
        if let Some(b) = data.get("easeBeforeKeyframe") {
            anim.ease_before = Some(as_bool(b));
        }
        if let Some(b) = data.get("disableAxisIfNotModified") {
            anim.disable_axis = as_bool(b);
        }
        if let Some(t) = data.get("beginTick").and_then(as_f32) {
            anim.begin_tick = Some(t);
        }
        if let Some(t) = data.get("endTick").and_then(as_f32) {
            anim.end_tick = Some(t);
        }
    }
    Ok(anim)
}

/// `calculateAnimationLength`.
pub fn length_of(bones: &BTreeMap<String, BoneAnim>) -> f32 {
    let mut length: f32 = 0.0;
    for b in bones.values() {
        length = length.max(b.rotation.last_time()).max(b.position.last_time()).max(b.scale.last_time()).max(last_time(&b.bend));
    }
    if length == 0.0 { f32::MAX } else { length }
}

fn events(a: &Map<String, Value>) -> Events {
    let mut e = Events::default();
    let key_time = |k: &str| k.parse::<f32>().unwrap_or(0.0) * 20.0;
    if let Some(Value::Object(s)) = a.get("sound_effects") {
        for (k, v) in s {
            e.sounds.push((key_time(k), v.get("effect").and_then(Value::as_str).unwrap_or_default().to_owned()));
        }
    }
    if let Some(Value::Object(s)) = a.get("particle_effects") {
        for (k, v) in s {
            let get = |n: &str| v.get(n).and_then(Value::as_str).unwrap_or_default().to_owned();
            e.particles.push((key_time(k), get("effect"), get("locator"), get("pre_effect_script")));
        }
    }
    if let Some(Value::Object(s)) = a.get("timeline") {
        for (k, v) in s {
            let text = match v {
                Value::String(t) => t.clone(),
                Value::Array(_) => v.to_string(),
                other => other.to_string(),
            };
            e.instructions.push((key_time(k), text));
        }
    }
    e
}

/// A keyframe's value, before it is split per axis.
type Entry = (f32, Value);

fn vec3(v: Value) -> Value {
    Value::Array(vec![v.clone(), v.clone(), v])
}

fn value_vec(v: &Value) -> Value {
    Value::Array(vec![v.clone(), Value::from(0), Value::from(0)])
}

/// `AnimationLoader.getKeyframes`.
fn keyframes(e: Option<&Value>) -> LoadResult<Vec<Entry>> {
    let Some(e) = e else { return Ok(Vec::new()) };
    match e {
        Value::Array(_) => Ok(vec![(0.0, e.clone())]),
        Value::Object(obj) => {
            if obj.contains_key("vector") {
                return Ok(vec![(0.0, e.clone())]);
            }
            if let Some(v) = obj.get("value") {
                let mut o = obj.clone();
                o.insert("vector".into(), value_vec(v));
                return Ok(vec![(0.0, Value::Object(o))]);
            }
            // Gson keeps a file's key order; timestamps come in rising order in practice.
            let mut entries: Vec<(&String, &Value)> = obj.iter().collect();
            entries.sort_by(|a, b| {
                let (x, y) = (a.0.parse::<f32>().unwrap_or(0.0), b.0.parse::<f32>().unwrap_or(0.0));
                x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut list: Vec<Entry> = Vec::new();
            for (k, v) in entries {
                let ts = k.parse::<f32>().unwrap_or(0.0);
                if ts == 0.0 && !list.is_empty() {
                    return Err(format!("Invalid keyframe data - multiple starting keyframes?{k}"));
                }
                let mut value = v.clone();
                if let Value::Object(o) = v {
                    if let Some(x) = o.get("value") {
                        let mut o2 = o.clone();
                        o2.insert("vector".into(), value_vec(x));
                        value = Value::Object(o2);
                        list.push((ts, value.clone()));
                    } else if !o.contains_key("vector") {
                        bedrock_keyframes(ts, o, &mut list)?;
                        continue;
                    }
                }
                list.push((ts, value));
            }
            Ok(list)
        }
        other => Ok(vec![(0.0, vec3(other.clone()))]),
    }
}

fn extract(k: &Value) -> LoadResult<Value> {
    match k {
        Value::Array(_) => Ok(k.clone()),
        Value::Object(o) => o
            .get("vector")
            .or_else(|| o.get("pre"))
            .or_else(|| o.get("post"))
            .cloned()
            .ok_or_else(|| format!("Invalid keyframe data - expected array or object, found {k}")),
        Value::Null => Err(format!("Invalid keyframe data - expected array or object, found {k}")),
        other => Ok(value_vec(other)),
    }
}

fn bedrock_keyframes(ts: f32, k: &Map<String, Value>, list: &mut Vec<Entry>) -> LoadResult<()> {
    let mut added = false;
    if let Some(pre) = k.get("pre") {
        added = true;
        let value = extract(pre)?;
        let at = if ts == 0.0 { ts } else { ts - 0.001 };
        if let Some(e) = k.get("easing") {
            let mut o = Map::new();
            o.insert("vector".into(), value);
            o.insert("easing".into(), e.clone());
            if let Some(a) = k.get("easingArgs") {
                o.insert("easingArgs".into(), a.clone());
            }
            list.push((at, Value::Object(o)));
        } else {
            list.push((at, value));
        }
    }
    if let Some(post) = k.get("post") {
        let values = extract(post)?;
        if let Some(l) = k.get("lerp_mode") {
            let mut o = Map::new();
            o.insert("vector".into(), values);
            o.insert("easing".into(), l.clone());
            list.push((ts, Value::Object(o)));
        } else {
            list.push((ts, values));
        }
        return Ok(());
    }
    if !added {
        return Err("Invalid keyframe data - expected array".into());
    }
    Ok(())
}

/// `MolangLoader.parseJson`: a value as Molang; a constant rotation in radians.
fn parse_value(rotation: bool, v: Option<&Value>, default: Exprs) -> Exprs {
    let Some(v) = v else { return default };
    let text = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => return default,
    };
    match molang::parse(&text) {
        Ok(list) => {
            if list.len() == 1 && rotation && molang::is_constant(&list[0]) {
                let deg = molang::Runtime::new().eval(&list);
                molang::constant(f64::from(deg).to_radians() as f32)
            } else {
                list
            }
        }
        Err(e) => {
            log::error!("[Emotecraft] Failed to compile molang '{text}': {e}");
            default
        }
    }
}

/// `pal.disabled` and `pal.skip` leave the axis out.
fn enabled(list: &Exprs) -> bool {
    if let [Expr::Access(o, p)] = list.as_slice()
        && let Expr::Ident(id) = &**o
        && id == "pal"
    {
        return p != "disabled" && p != "skip";
    }
    true
}

fn easing_of(obj: Option<&Map<String, Value>>, axis: &str, default: Easing) -> Easing {
    match obj.and_then(|o| o.get(&format!("easing{axis}"))) {
        Some(Value::String(s)) => Easing::from_string(&s.to_lowercase()),
        Some(_) => Easing::Linear,
        None => default,
    }
}

fn args_of(obj: Option<&Map<String, Value>>, axis: &str, default: &[Exprs]) -> Vec<Exprs> {
    match obj.and_then(|o| o.get(&format!("easingArgs{axis}"))).and_then(Value::as_array) {
        Some(a) => a.iter().map(|v| molang::constant(as_f32(v).unwrap_or(0.0))).collect(),
        None => default.to_vec(),
    }
}

fn stack(entries: Vec<Entry>, ty: Ty) -> LoadResult<Stack> {
    let mut s = Stack::default();
    if entries.is_empty() {
        return Ok(s);
    }
    let rotation = matches!(ty, Ty::Rotation | Ty::Bend);
    let default = if ty == Ty::Scale { one() } else { zero() };
    let mut prev: [Option<Exprs>; 3] = [None, None, None];
    let mut prev_time = [0.0f32; 3];
    let axes = ["X", "Y", "Z"];
    for (time, element) in &entries {
        let vector = match element {
            Value::Array(a) => a.clone(),
            Value::Object(o) => o.get("vector").and_then(Value::as_array).cloned().ok_or("Missing vector, expected to find a JsonArray")?,
            _ => return Err("bad keyframe".into()),
        };
        let obj = element.as_object();
        let easing = easing_of(obj, "", Easing::Linear);
        let args = args_of(obj, "", &[]);
        for i in 0..3 {
            let value = parse_value(rotation, vector.get(i), default.clone());
            if !enabled(&value) {
                continue;
            }
            let start = prev[i].clone().unwrap_or_else(|| value.clone());
            s.axis_mut(i).push(Keyframe::new((time - prev_time[i]) * 20.0, start, value.clone(), easing_of(obj, axes[i], easing), args_of(obj, axes[i], &args)));
            prev[i] = Some(value);
            prev_time[i] = *time;
        }
    }
    for i in 0..3 {
        add_args(s.axis_mut(i), ty)?;
    }
    Ok(s)
}

fn to_radians_for_bezier(e: Exprs) -> Exprs {
    if molang::single_constant(&e) {
        let v = molang::Runtime::new().eval(&e);
        return molang::constant(f64::from(v).to_radians() as f32);
    }
    log::warn!("[Emotecraft] Invalid easing arguments for bezier: {e:?}\nFor rotations bezier args can only be floats.");
    e
}

/// `addArgsForKeyframes`: Catmull-Rom's neighbours and Blockbench's bezier handles.
fn add_args(frames: &mut [Keyframe], ty: Ty) -> LoadResult<()> {
    if frames.is_empty() {
        return Ok(());
    }
    if frames.len() == 1 && frames[0].easing != Easing::Linear {
        let f = &mut frames[0];
        f.easing = Easing::Linear;
        f.args = Vec::new();
        return Ok(());
    }
    for i in 0..frames.len() {
        match frames[i].easing {
            Easing::CatmullRom => {
                let before = if i == 0 { frames[i].start.clone() } else { frames[i - 1].end.clone() };
                let after = if i + 1 >= frames.len() { frames[i].end.clone() } else { frames[i + 1].end.clone() };
                frames[i].args = vec![before, after];
            }
            Easing::Bezier => {
                let args = frames[i].args.clone();
                if args.len() < 4 {
                    return Err("Index out of bounds: bezier easing needs 4 arguments".into());
                }
                let mut left = args[0].clone();
                let mut right = args[2].clone();
                let right_time = args[3].clone();
                if ty == Ty::Rotation {
                    right = to_radians_for_bezier(right);
                    left = to_radians_for_bezier(left);
                }
                let mut new_args = vec![left, args[1].clone()];
                if args.len() > 4 {
                    new_args.push(args[4].clone());
                    new_args.push(args.get(5).cloned().unwrap_or_default());
                }
                frames[i].args = new_args;
                if i + 1 < frames.len() {
                    let next = &mut frames[i + 1];
                    if next.easing != Easing::Bezier {
                        next.easing = Easing::Bezier;
                        next.args = vec![zero(), zero(), right, right_time];
                    } else {
                        next.args.push(right);
                        next.args.push(right_time);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_names() {
        assert_eq!(bone_name("rightArm"), "right_arm");
        assert_eq!(bone_name("body_control"), "body_control");
        assert_eq!(bone_name("leftItem"), "left_item");
    }

    #[test]
    fn legacy_waving() {
        let text = r#"{"name":"w","emote":{"beginTick":12,"endTick":45,"stopTick":50,"degrees":true,
            "moves":[{"tick":0,"easing":"inQuad","torso":{"roll":0},"rightLeg":{"roll":0,"x":-1.9,"y":12}},
            {"tick":10,"easing":"InOutSine","rightArm":{"roll":185}}]}}"#;
        let a = &load_json(text).unwrap()[0];
        assert_eq!(a.format, Format::PlayerAnimator);
        assert!(a.apply_bend);
        assert_eq!(a.length, 50.0);
        assert_eq!(a.begin_tick, Some(12.0));
        assert_eq!(a.end_tick, Some(45.0));
        // v1: torso is the body root.
        assert!(a.bones.contains_key("body"));
        let arm = &a.bones["right_arm"];
        // easeBeforeKeyframe false: shifted, plus the 0.001 tail.
        assert_eq!(arm.rotation.z.len(), 2);
        assert_eq!(arm.rotation.z[0].easing, Easing::InOutSine);
        assert_eq!(arm.rotation.z[0].length, 10.0);
        let leg = &a.bones["right_leg"];
        // x - default, 0 here; y - default and negated.
        assert_eq!(leg.position.x[0].end, molang::constant(0.0));
    }
}
