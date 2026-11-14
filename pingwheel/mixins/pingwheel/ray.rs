// Ping Wheel's ray (`Raycast.traceDirectional`): from the camera entity's eyes along its view,
// blocks by their visual shape and no fluids (`ClipContext.Block.VISUAL`, `Fluid.NONE`), or while
// crouching by their outline and every fluid (`OUTLINE`, `ANY`); then the entities along the ray,
// each box grown by a quarter block, the nearest hit winning over the block's. Spliced into
// game.rs as `pingwheel_ray`.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::Game;
use horizon_client::blocks::BlockFacts;
use horizon_client::collisions::SceneCollisions;
use horizon_core::geometry::{Aabb, ShapeOp, Vec3, VoxelShape};
use horizon_core::position::BlockPos;
use horizon_world::collision_context::{CollisionContext, CollisionEntity, CollisionItem, EntityContextKind};
use horizon_world::entity_obstruction::EntityIdentity;
use horizon_world::registry::BlockStateId;
use horizon_world::world_ray::{RayBlockMode, RayFluidMode, RayWorld, clip_world};
use std::io;

/// What the ray hit: the point, and the entity's UUID for an entity.
pub(super) struct Hit {
    pub location: [f64; 3],
    pub entity: Option<u128>,
}

/// `Level.clip` over the window's world in Ping Wheel's two modes.
struct Rays<'a, 'r> {
    game: &'a Game<'r>,
    scene: SceneCollisions<'a, 'r>,
    facts: &'a BlockFacts,
    context: CollisionContext<'static>,
    /// Crouching: outlines and fluids.
    outline: bool,
}

fn shape_of(boxes: impl IntoIterator<Item = [f64; 6]>) -> io::Result<VoxelShape> {
    let mut shape = VoxelShape::empty();
    for b in boxes {
        let part = VoxelShape::from_box(Aabb::new(Vec3::new(b[0], b[1], b[2]), Vec3::new(b[3], b[4], b[5]))).map_err(|e| io::Error::other(format!("{e:?}")))?;
        shape = shape.join(&part, ShapeOp::OR).map_err(|e| io::Error::other(format!("{e:?}")))?;
    }
    Ok(shape)
}

impl RayWorld for Rays<'_, '_> {
    type Error = io::Error;
    type Block = BlockStateId;
    /// The fluid's height in the block, if there is a fluid.
    type Fluid = Option<f32>;
    fn block_state(&mut self, p: BlockPos) -> io::Result<BlockStateId> {
        Ok(self.scene.world.block(p.x, p.y, p.z))
    }
    fn fluid_state(&mut self, p: BlockPos) -> io::Result<Option<f32>> {
        if !self.outline {
            return Ok(None);
        }
        Ok(self.game.fluid_height_of(p.x, p.y, p.z, None).map(|(h, _)| h))
    }
    fn block_shape(&mut self, state: BlockStateId, p: BlockPos, _: RayBlockMode) -> io::Result<VoxelShape> {
        if self.outline {
            // `getShape`: the outline, as the crosshair's block outline is.
            let boxes = self.game.outline_boxes([p.x, p.y, p.z]);
            let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
            return shape_of(boxes.into_iter().map(|b| [b[0] - x, b[1] - y, b[2] - z, b[3] - x, b[4] - y, b[5] - z]));
        }
        // `getVisualShape`: the collision shape but for the blocks that override it (glass,
        // panes, bars, grates and powder snow see-through; soul sand and mud full; snow layers and
        // fences their outline), as the game's camera ray has it.
        let facts = self.facts.get(state);
        let name = &*facts.name;
        if name.ends_with("glass") || name.ends_with("glass_pane") || name.ends_with("_bars") || name.ends_with("copper_grate") || name == "minecraft:powder_snow" {
            return Ok(VoxelShape::empty());
        }
        if name == "minecraft:soul_sand" || name == "minecraft:mud" {
            return Ok(VoxelShape::block());
        }
        if name == "minecraft:snow" || name.ends_with("_fence") {
            return shape_of(facts.outline.iter().map(|b| b.map(f64::from)));
        }
        let binding = self.scene.binding;
        binding.collision_shape_with_context(&mut self.scene, state, p, &self.context).map_err(|e| io::Error::other(format!("{e:?}")))
    }
    fn interaction_shape(&mut self, _: BlockStateId, _: BlockPos) -> io::Result<VoxelShape> {
        Ok(VoxelShape::empty())
    }
    fn fluid_is_source(&mut self, _: Option<f32>) -> io::Result<bool> {
        Ok(false)
    }
    fn fluid_is_empty(&mut self, f: Option<f32>) -> io::Result<bool> {
        Ok(f.is_none())
    }
    fn fluid_is_water(&mut self, _: Option<f32>) -> io::Result<bool> {
        Ok(false)
    }
    fn fluid_shape(&mut self, f: Option<f32>, _: BlockPos) -> io::Result<VoxelShape> {
        match f {
            Some(h) if h >= 1.0 => Ok(VoxelShape::block()),
            Some(h) => shape_of([[0.0, 0.0, 0.0, 1.0, f64::from(h), 1.0]]),
            None => Ok(VoxelShape::empty()),
        }
    }
}

fn v(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// A box grown by `getPickRadius()` (0) and 0.25, clipped by the ray.
fn entity_clip(min: [f64; 3], max: [f64; 3], from: Vec3, to: Vec3) -> Option<Vec3> {
    let g = 0.25;
    Aabb::new(Vec3::new(min[0] - g, min[1] - g, min[2] - g), Vec3::new(max[0] + g, max[1] + g, max[2] + g)).clip(from, to)
}

fn corners(b: Aabb) -> ([f64; 3], [f64; 3]) {
    let (lo, hi) = (b.min_position(), b.max_position());
    ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z])
}

impl Game<'_> {
    /// `Raycast.traceDirectional` along the view for `distance` blocks; None on a miss.
    pub(super) fn pingwheel_trace(&self, distance: f64) -> Option<Hit> {
        let (eye, dir) = self.aim()?;
        let crouching = self.player.as_ref().is_some_and(|p| p.pose == horizon_client::pose::Pose::Crouching);
        let start = v(eye);
        let end = v([eye[0] + dir[0] * distance, eye[1] + dir[1] * distance, eye[2] + dir[2] * distance]);
        let context = CollisionContext::for_entity(
            CollisionEntity { identity: EntityIdentity(0), kind: EntityContextKind::Player, y: eye[1], descending: crouching, main_hand: Some(CollisionItem::EMPTY) },
            false,
        );
        let mut rays = Rays {
            game: self,
            scene: SceneCollisions { world: &self.world, binding: self.collision, subject: None },
            facts: self.res.facts(),
            context,
            outline: crouching,
        };
        let fluids = if crouching { RayFluidMode::Any } else { RayFluidMode::None };
        let mode = if crouching { RayBlockMode::Outline } else { RayBlockMode::Visual };
        let block = clip_world(&mut rays, start, end, mode, fluids, 8192).ok();
        // A miss's location is the ray's end, as `BlockHitResult.miss` has it.
        let block_at = block.map_or(end, |r| r.hit.location);
        let block_hit = block.is_some_and(|r| !r.miss);

        // `traceEntity`: every entity but the camera's and spectators, the nearest hit.
        let mut best: Option<(f64, Vec3, Option<u128>)> = None;
        let max = start.distance_squared(end);
        let uuid = |id: u32| self.persist.uuids.get(&id).map(|i| horizon_client::lan::playerdata::ints_uuid(*i));
        let mut offer = |hit: Option<Vec3>, id: Option<u128>| {
            if let Some(p) = hit {
                let d = start.distance_squared(p);
                if d < max && best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, p, id));
                }
            }
        };
        for m in self.mobs.iter() {
            if m.extra.spectator {
                continue;
            }
            let (lo, hi) = corners(m.bounds());
            offer(entity_clip(lo, hi, start, end), uuid(m.id).or_else(|| self.remote_player_uuid(m.id)));
        }
        for i in self.dropped.iter() {
            let (lo, hi) = corners(i.bounds());
            offer(entity_clip(lo, hi, start, end), uuid(i.id));
        }
        for b in self.boats.iter() {
            let (lo, hi) = corners(b.bounds());
            offer(entity_clip(lo, hi, start, end), uuid(b.id));
        }
        for c in self.carts.iter() {
            let (lo, hi) = corners(c.bounds());
            offer(entity_clip(lo, hi, start, end), uuid(c.id));
        }
        for a in self.armor_stands.iter() {
            let (lo, hi) = corners(a.bounds());
            offer(entity_clip(lo, hi, start, end), uuid(a.id));
        }
        for f in self.item_frames.iter() {
            let (lo, hi) = f.bounds();
            offer(entity_clip(lo, hi, start, end), uuid(f.id));
        }
        match best {
            Some((d, p, id)) if !block_hit || start.distance_squared(block_at) >= d => Some(Hit { location: [p.x, p.y, p.z], entity: id }),
            _ if block_hit => Some(Hit { location: [block_at.x, block_at.y, block_at.z], entity: None }),
            _ => None,
        }
    }

    /// Where a pinged entity is drawn now: its position at this frame's partial tick, lifted by
    /// its box's height (`getPosition(tickDelta).add(0, getBoundingBox().getYsize(), 0)`), and
    /// its stack when it is a dropped item.
    pub(super) fn pingwheel_entity(&self, entity: u128, partial: f64) -> Option<([f64; 3], Option<horizon_client::items::ItemStack>)> {
        let id = self.persist.uuids.iter().find(|(_, i)| horizon_client::lan::playerdata::ints_uuid(**i) == entity).map(|(id, _)| *id);
        let lerp = |prev: Vec3, pos: Vec3| [prev.x + (pos.x - prev.x) * partial, prev.y + (pos.y - prev.y) * partial, prev.z + (pos.z - prev.z) * partial];
        let height = |b: Aabb| b.max_position().y - b.min_position().y;
        if let Some(id) = id {
            if let Some(i) = self.dropped.iter().find(|i| i.id == id) {
                let p = lerp(i.previous, i.position);
                return Some(([p[0], p[1] + height(i.bounds()), p[2]], Some(i.stack.clone())));
            }
            if let Some(m) = self.mobs.iter().find(|m| m.id == id) {
                let p = lerp(m.previous, m.position);
                return Some(([p[0], p[1] + height(m.bounds()), p[2]], None));
            }
            if let Some(b) = self.boats.iter().find(|b| b.id == id) {
                let p = b.position;
                return Some(([p.x, p.y + height(b.bounds()), p.z], None));
            }
            if let Some(c) = self.carts.iter().find(|c| c.id == id) {
                let p = c.position;
                return Some(([p.x, p.y + height(c.bounds()), p.z], None));
            }
            if let Some(a) = self.armor_stands.iter().find(|a| a.id == id) {
                let p = a.position;
                return Some(([p.x, p.y + height(a.bounds()), p.z], None));
            }
        }
        // Another player the client knows by UUID only through the player list.
        let m = self.mobs.iter().find(|m| self.remote_player_uuid(m.id) == Some(entity))?;
        let p = lerp(m.previous, m.position);
        Some(([p[0], p[1] + height(m.bounds()), p[2]], None))
    }
}
