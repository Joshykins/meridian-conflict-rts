//! Mesh-building toolkit for the procedural models.
//!
//! A [`MeshBuilder`] is a brush: it carries a current material, part and
//! transform, and every primitive is emitted with them. All primitives are
//! flat shaded (each face owns its vertices), which suits the hard-edged
//! look, and get box-projected UVs in metres.
//!
//! Every solid is built as a *loft*: a stack of point rings joined by quads
//! and closed by two caps. After the current transform is applied the solid's
//! signed volume decides its orientation, so faces wind counter-clockwise
//! seen from outside whatever the ring order, and mirrored transforms
//! (see [`MeshBuilder::mirror_y`]) need no special handling.

use std::f32::consts::{PI, TAU};

use glam::{Affine3A, Vec2, Vec3};

use super::{material, part, pattern, rig, Legs, MeshLod, MeshVertex, Treads, LOD_COUNT};

/// Triangles smaller than this (m²) are dropped instead of emitted.
const MIN_TRIANGLE_AREA: f32 = 2.0e-5;
/// Points closer than this (m) are merged when a face is emitted.
const WELD_DISTANCE: f32 = 1.0e-4;

/// One horizontal slice of a [`MeshBuilder::loft_z`] solid.
#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub z: f32,
    /// Scale of the plan profile about the profile origin.
    pub scale: Vec2,
    /// Offset of the scaled profile.
    pub shift: Vec2,
}

impl Section {
    pub fn new(z: f32, scale: f32) -> Self {
        Section {
            z,
            scale: Vec2::splat(scale),
            shift: Vec2::ZERO,
        }
    }

    pub fn scaled(z: f32, scale_x: f32, scale_y: f32) -> Self {
        Section {
            z,
            scale: Vec2::new(scale_x, scale_y),
            shift: Vec2::ZERO,
        }
    }

    pub fn shifted(mut self, x: f32, y: f32) -> Self {
        self.shift = Vec2::new(x, y);
        self
    }
}

pub struct MeshBuilder {
    lod: usize,
    mesh: MeshLod,
    material: u32,
    pattern: u32,
    part: u32,
    rig: u32,
    /// The next loft is a tube: its sides share one frame that goes right round it.
    round: bool,
    /// How the face being emitted gets its [`MeshVertex::face`] frame.
    framing: Framing,
    transform: Affine3A,
    turret_pivot: Vec3,
    spinner_pivot: Vec3,
    treads: Option<Treads>,
    legs: Option<Legs>,
    hover: bool,
    arm_pivot: Option<[f32; 3]>,
    arm_boom: bool,
    recoil: Option<[f32; 4]>,
    fold: Option<[f32; 4]>,
    fold_wrist: Option<[f32; 4]>,
    neck: Option<[f32; 2]>,
    mount: Option<[f32; 4]>,
    houses: Vec<super::House>,
    spins: Vec<(u32, u32, [f32; 3])>,
    pit: Option<super::Pit>,
    dust_line: Option<f32>,
    /// Keys of the unit's refit modules, in look-bit order (`mc_data::Module::bit`).
    modules: Vec<String>,
    /// Index ranges of the closed solids, so tests can check each one's orientation.
    #[cfg(test)]
    solids: Vec<std::ops::Range<usize>>,
}

impl MeshBuilder {
    /// A builder for level of detail `lod` (0 = full) whose root transform is `root`.
    pub fn new(lod: usize, root: Affine3A) -> Self {
        assert!(lod < LOD_COUNT);
        MeshBuilder {
            lod,
            mesh: MeshLod::default(),
            material: material::PLATING,
            pattern: pattern::GENERIC,
            part: part::HULL,
            rig: 0,
            round: false,
            framing: Framing::Flat,
            transform: root,
            turret_pivot: root.transform_point3(Vec3::ZERO),
            spinner_pivot: root.transform_point3(Vec3::ZERO),
            treads: None,
            legs: None,
            hover: false,
            arm_pivot: None,
            arm_boom: false,
            recoil: None,
            fold: None,
            fold_wrist: None,
            neck: None,
            mount: None,
            houses: Vec::new(),
            spins: Vec::new(),
            pit: None,
            dust_line: None,
            modules: Vec::new(),
            #[cfg(test)]
            solids: Vec::new(),
        }
    }

    pub fn finish(self) -> MeshLod {
        self.mesh
    }

    // ---- level of detail -------------------------------------------------

    pub fn lod(&self) -> usize {
        self.lod
    }

    /// Full detail only: greebles, glow strips, antennae.
    pub fn fine(&self) -> bool {
        self.lod == 0
    }

    /// Everything except the box-silhouette level.
    pub fn mid(&self) -> bool {
        self.lod <= 1
    }

    /// The last level: a handful of boxes.
    pub fn coarse(&self) -> bool {
        self.lod == LOD_COUNT - 1
    }

    /// Side count for a round shape that has `n` sides at full detail.
    pub fn sides(&self, n: usize) -> usize {
        match self.lod {
            0 => n,
            1 => (n * 3 / 4).max(4),
            _ => 4,
        }
    }

    // ---- brush -----------------------------------------------------------

    /// Sets the material, and puts the pattern back to the fitted generic one.
    pub fn paint(&mut self, material: u32) -> &mut Self {
        self.material = material;
        self.pattern = pattern::GENERIC;
        self
    }

    /// What the shader draws on the faces that follow ([`pattern`]), until the next [`Self::paint`].
    pub fn pattern(&mut self, pattern: u32) -> &mut Self {
        self.pattern = pattern;
        self
    }

    /// Runs `f` with vertices assigned to `part`, then restores the previous part.
    pub fn with_part(&mut self, part: u32, f: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.part, part);
        f(self);
        self.part = previous;
    }

    /// Runs `f` with vertices riding leg bone `limb` (`rig::THIGH`, `SHIN` or `FOOT`).
    pub fn with_limb(&mut self, limb: u32, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig = (previous & !rig::LIMB_MASK) | limb;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as build-arm gear that works while the unit builds (`rig::WORK_*`).
    pub fn with_work(&mut self, work: u32, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig = (previous & !rig::WORK_MASK) | (work & rig::WORK_MASK);
        f(self);
        self.rig = previous;
    }

    /// Runs `f` with vertices that slide back along the barrel when the gun fires.
    pub fn with_recoil(&mut self, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig |= rig::RECOIL;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as a hover skirt: dropped on water, tucked up on land.
    pub fn with_float(&mut self, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig |= rig::FLOAT;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as a factory build deck: up while a unit is printing, then
    /// lowered to let it roll out. Matches `RIG_LIFT` / the vertex shader drop.
    pub fn with_lift(&mut self, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig |= rig::LIFT;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as plantable gear: folded up on the move, down when deployed.
    pub fn with_deploy(&mut self, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig |= rig::DEPLOY;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as part of what the unit's upgrade adds: hidden until the refit
    /// begins, going up `at` (zero to one) of the way through it.
    pub fn upgrade(&mut self, at: f32, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig = (previous & rig::LIMB_MASK)
            | rig::UPGRADE
            | ((at.clamp(0.0, 1.0) * 255.0) as u32) << rig::UPGRADE_AT_SHIFT;
        f(self);
        self.rig = previous;
    }

    /// The keys of the unit's refit modules, in look-bit order, so [`Self::module`] can tag pieces.
    pub fn set_modules(&mut self, keys: &[&str]) {
        self.modules = keys.iter().map(|k| (*k).to_owned()).collect();
    }

    fn module_tag(&self, key: &str) -> Option<u32> {
        self.modules.iter().position(|k| k == key).map(|i| i as u32 + 1)
    }

    /// Runs `f` as pieces of refit module `key`: drawn only on a unit that has it
    /// fitted (or a later tier over it), and raised `at` (zero to one) of the way
    /// through the refit that fits it. Nothing is emitted when the unit has no such module.
    pub fn module(&mut self, key: &str, at: f32, f: impl FnOnce(&mut Self)) {
        let Some(tag) = self.module_tag(key) else {
            return;
        };
        let previous = self.rig;
        self.rig = (previous & !(rig::MODULE_MASK | rig::UPGRADE_AT_MASK))
            | tag << rig::MODULE_SHIFT
            | ((at.clamp(0.0, 1.0) * 255.0) as u32) << rig::UPGRADE_AT_SHIFT;
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as pieces that are taken off when module `key` goes on: what it replaces.
    /// Nothing changes when the unit has no such module.
    pub fn until(&mut self, key: &str, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        if let Some(tag) = self.module_tag(key) {
            self.rig = (previous & !rig::UNTIL_MASK) | tag << rig::UNTIL_SHIFT;
        }
        f(self);
        self.rig = previous;
    }

    /// Runs `f` as gear that folds away about `hinge` (current frame) when the
    /// unit is not building: authored out, folded `stowed` radians back (pitched up and over).
    pub fn with_fold(&mut self, hinge: Vec3, stowed: f32, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(hinge);
        self.fold = Some([at.x, at.y, at.z, stowed]);
        self.with_limb(rig::FOLD, f);
    }

    /// Runs `f` as the head on the end of the `with_fold` gear, pitching about `wrist`
    /// (current frame): authored out and pointing at the work, folded `stowed` radians
    /// (negative: down and back along the arm) when stowed. It rides the gear's arm.
    pub fn with_fold_head(&mut self, wrist: Vec3, stowed: f32, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(wrist);
        self.fold_wrist = Some([at.x, at.y, at.z, stowed]);
        self.with_limb(rig::FOLD_HEAD, f);
    }

    /// Sinks the hips `crouch` metres in full stride (`Legs::crouch`). After `set_legs`.
    pub fn set_walk_crouch(&mut self, crouch: f32) {
        let legs = self.legs.as_mut().expect("set_legs first");
        legs.crouch = self.transform.transform_vector3(Vec3::Z * crouch).z;
    }

    /// Runs `f` as a walker's head, turning about a neck at `neck` (current frame; its y
    /// is taken as the centreline) while the unit stands idle.
    pub fn with_head(&mut self, neck: Vec3, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(neck);
        self.neck = Some([at.x, at.z]);
        self.with_limb(rig::HEAD, f);
    }

    /// Where the model's head turns, if it has one (`with_head`).
    pub fn neck(&self) -> Option<[f32; 2]> {
        self.neck
    }

    /// Wrist and stowed angle of the head on the model's folding gear, if it has one.
    pub fn fold_wrist(&self) -> Option<[f32; 4]> {
        self.fold_wrist
    }

    /// Runs `f` as a turret of its own on the turret, turning and pitching about `pivot`
    /// (current frame). Authored level and facing forward; `with_recoil` inside it kicks
    /// `travel` metres back along x.
    pub fn with_mount(&mut self, pivot: Vec3, travel: f32, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(pivot);
        let side = self.transform.transform_vector3(Vec3::X).length();
        self.mount = Some([at.x, at.y, at.z, travel * side]);
        self.with_limb(rig::MOUNT, f);
    }

    pub fn mount(&self) -> Option<[f32; 4]> {
        self.mount
    }

    /// Runs `f` as a gun house of its own on the hull, bound to `weapon` of the unit: it
    /// turns about `pivot` (current frame) by that weapon's yaw off the hull, and what is
    /// inside `with_recoil` pitches about the pivot by the weapon's pitch and kicks `travel`
    /// metres back along x when it fires. Authored level and facing forward (+x), even a
    /// house astern: a `rear` weapon rests turned round. Up to `rig::HOUSE_COUNT` houses.
    pub fn with_house(&mut self, weapon: usize, pivot: Vec3, travel: f32, f: impl FnOnce(&mut Self)) {
        let slot = self
            .houses
            .iter()
            .position(|h| h.weapon as usize == weapon && h.pivot == self.transform.transform_point3(pivot).to_array())
            .unwrap_or_else(|| {
                assert!((self.houses.len() as u32) < rig::HOUSE_COUNT, "too many gun houses");
                let at = self.transform.transform_point3(pivot);
                let side = self.transform.transform_vector3(Vec3::X).length();
                self.houses.push(super::House { pivot: at.to_array(), travel: travel * side, weapon: weapon as u8 });
                self.houses.len() - 1
            });
        self.with_limb(rig::HOUSE_FIRST + slot as u32, f);
    }

    pub fn houses(&self) -> Vec<super::House> {
        self.houses.clone()
    }

    /// Runs `f` as rotary barrels turning about the line through `axis` (current frame)
    /// along x. The axis counts for the module tags in force here.
    pub fn with_spin(&mut self, axis: Vec3, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(axis);
        let need = (self.rig & rig::MODULE_MASK) >> rig::MODULE_SHIFT;
        let until = (self.rig & rig::UNTIL_MASK) >> rig::UNTIL_SHIFT;
        if !self.spins.iter().any(|s| s.0 == need && s.1 == until) {
            self.spins.push((need, until, at.to_array()));
        }
        let previous = self.rig;
        self.rig |= rig::SPIN;
        f(self);
        self.rig = previous;
    }

    pub fn spins(&self) -> Vec<(u32, u32, [f32; 3])> {
        self.spins.clone()
    }

    /// Hinge and stowed angle of the model's folding gear, if it has any.
    pub fn fold(&self) -> Option<[f32; 4]> {
        self.fold
    }

    /// Runs `f` inside the local frame `local` (composed onto the current transform).
    pub fn with(&mut self, local: Affine3A, f: impl FnOnce(&mut Self)) {
        let previous = self.transform;
        self.transform = previous * local;
        f(self);
        self.transform = previous;
    }

    pub fn at(&mut self, offset: Vec3, f: impl FnOnce(&mut Self)) {
        self.with(Affine3A::from_translation(offset), f);
    }

    /// A frame at `pivot` whose +x axis is pitched up by `angle` radians: for
    /// elevated barrels and raked racks.
    pub fn pitched(&mut self, pivot: Vec3, angle: f32, f: impl FnOnce(&mut Self)) {
        self.with(
            Affine3A::from_translation(pivot) * Affine3A::from_rotation_y(-angle),
            f,
        );
    }

    /// A frame at `pivot` yawed counter-clockwise (seen from above) by `angle` radians.
    pub fn yawed(&mut self, pivot: Vec3, angle: f32, f: impl FnOnce(&mut Self)) {
        self.with(
            Affine3A::from_translation(pivot) * Affine3A::from_rotation_z(angle),
            f,
        );
    }

    /// Emits `f` twice: as written, and mirrored to the other side (y negated).
    pub fn mirror_y(&mut self, f: impl Fn(&mut Self)) {
        f(self);
        self.with(Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)), |b| f(b));
    }

    /// Emits `f` `count` times, each turned a further `1/count` of a turn about the z axis.
    pub fn radial(&mut self, count: usize, f: impl Fn(&mut Self)) {
        for i in 0..count {
            self.with(
                Affine3A::from_rotation_z(TAU * i as f32 / count as f32),
                |b| f(b),
            );
        }
    }

    /// Records the turret yaw axis (given in the current frame).
    pub fn set_turret_pivot(&mut self, pivot: Vec3) {
        self.turret_pivot = self.transform.transform_point3(pivot);
    }

    /// Height (current frame) up to which a vehicle wears the field dust thrown up by its
    /// running gear: its lower hull. Unset, that is 62% of the model's height, which is
    /// wrong for a low hull under a tall mount (`Model::dust_line`).
    pub fn set_dust_line(&mut self, z: f32) {
        self.dust_line = Some(self.transform.transform_point3(Vec3::Z * z).z);
    }

    pub fn dust_line(&self) -> Option<f32> {
        self.dust_line
    }

    /// Records a pit dug into the ground (`Model::pit`), given in the current frame.
    pub fn set_pit(&mut self, pit: super::Pit) {
        let t = self.transform;
        let up = |d: f32| t.transform_vector3(Vec3::new(0.0, 0.0, d)).length();
        let rack = t.transform_point3(Vec3::new(pit.rack[0], pit.rack[1], 0.0));
        self.pit = Some(super::Pit {
            open: t.transform_point3(Vec3::new(0.0, 0.0, pit.open)).z,
            radius: t.transform_vector3(Vec3::new(pit.radius, 0.0, 0.0)).length(),
            stroke: up(pit.stroke),
            section: up(pit.section),
            rack: [rack.x, rack.y],
            afloat_lift: up(pit.afloat_lift),
        });
    }

    pub fn pit(&self) -> Option<super::Pit> {
        self.pit
    }

    /// Records the spinner axis (given in the current frame).
    pub fn set_spinner_pivot(&mut self, pivot: Vec3) {
        self.spinner_pivot = self.transform.transform_point3(pivot);
    }

    /// Records where the tracks touch the ground (given in the current frame),
    /// for the marks and dust they leave: the centre line of the left track
    /// (y), a track's width, and the x of the tracks' rear end.
    pub fn set_treads(&mut self, center_y: f32, width: f32, rear: f32) {
        let side = self.transform.transform_vector3(Vec3::Y).length();
        self.treads = Some(Treads {
            half_gauge: center_y * side,
            width: width * side,
            rear: self.transform.transform_point3(Vec3::X * rear).x,
        });
    }

    /// Records the left leg's joints at rest (given in the current frame), the
    /// ground one full cycle covers, the share of it a foot is planted for and
    /// how high a foot lifts. Everything emitted `with_limb` is then posed by
    /// the vertex shader as the unit walks.
    pub fn set_legs(
        &mut self,
        hip: Vec3,
        knee: Vec3,
        ankle: Vec3,
        stride: f32,
        stance: f32,
        lift: f32,
    ) {
        assert!(
            stride > 0.0 && stride.log2().fract() == 0.0,
            "a stride is a power of two metres"
        );
        assert!((0.2..0.9).contains(&stance));
        let at = |p: Vec3| self.transform.transform_point3(p).to_array();
        self.legs = Some(Legs {
            hip: at(hip),
            knee: at(knee),
            ankle: at(ankle),
            stride,
            stance,
            lift: self.transform.transform_vector3(Vec3::Z * lift).z,
            crouch: 0.0,
            foot: [0.0; 3],
        });
    }

    /// Records the sole of each foot (given in the current frame), for the
    /// marks a walker leaves: metres behind the ankle, ahead of it, and the
    /// sole's width. The right foot is the left's mirror.
    pub fn set_foot(&mut self, rear: f32, front: f32, width: f32) {
        let along = self.transform.transform_vector3(Vec3::X).length();
        let side = self.transform.transform_vector3(Vec3::Y).length();
        if let Some(legs) = &mut self.legs {
            legs.foot = [rear * along, front * along, width * side];
        }
    }

    /// Records the left elbow (given in the current frame): forearms emitted
    /// `with_limb(rig::ARM_GUN | ARM_TOOL)` pitch about it and its mirror image.
    pub fn set_arm_pivot(&mut self, pivot: Vec3) {
        self.arm_pivot = Some(self.transform.transform_point3(pivot).to_array());
    }

    pub fn arm_pivot(&self) -> Option<[f32; 3]> {
        self.arm_pivot
    }

    /// Elbow of a two-bone build arm. The shoulder is the turret pivot; the
    /// boom (`rig::ARM_BOOM`) pitches about it and carries the tool forearm.
    pub fn set_arm_boom(&mut self, elbow: Vec3) {
        self.set_arm_pivot(elbow);
        self.arm_boom = true;
    }

    pub fn arm_boom(&self) -> bool {
        self.arm_boom
    }

    /// Records the rest-space barrel axis from `breech` to `muzzle` and how far
    /// `with_recoil` verts kick back along it, in the current frame.
    pub fn set_recoil(&mut self, breech: Vec3, muzzle: Vec3, travel: f32) {
        let axis = (muzzle - breech).try_normalize().unwrap_or(Vec3::X);
        let dir = self.transform.transform_vector3(axis);
        let length = dir.length();
        if length < 1e-5 {
            return;
        }
        let dir = dir / length;
        let travel = self.transform.transform_vector3(axis * travel).length();
        self.recoil = Some([dir.x, dir.y, dir.z, travel]);
    }

    pub fn recoil(&self) -> Option<[f32; 4]> {
        self.recoil
    }

    pub fn legs(&self) -> Option<Legs> {
        self.legs
    }

    pub fn treads(&self) -> Option<Treads> {
        self.treads
    }

    /// Marks this hull as a hovercraft: the shader bobs it, the renderer
    /// raises downwash, and the skirt is left sitting off the ground.
    /// Tread crawl and the running-gear shudder do not apply.
    pub fn set_hover(&mut self) {
        self.hover = true;
    }

    pub fn hover(&self) -> bool {
        self.hover
    }

    pub fn turret_pivot(&self) -> Vec3 {
        self.turret_pivot
    }

    pub fn spinner_pivot(&self) -> Vec3 {
        self.spinner_pivot
    }

    // ---- boxes -----------------------------------------------------------

    /// Axis-aligned box.
    pub fn cuboid(&mut self, center: Vec3, size: Vec3) {
        let base = center - Vec3::Z * (size.z * 0.5);
        self.frustum(base, size.truncate(), size.truncate(), size.z, Vec2::ZERO);
    }

    /// Axis-aligned box from its two corners.
    pub fn block(&mut self, min: Vec3, max: Vec3) {
        self.cuboid((min + max) * 0.5, max - min);
    }

    /// Box without its bottom face, for shapes that sit on the ground or on another solid.
    pub fn cuboid_open(&mut self, center: Vec3, size: Vec3) {
        let base = center - Vec3::Z * (size.z * 0.5);
        self.frustum_open(base, size.truncate(), size.truncate(), size.z, Vec2::ZERO);
    }

    /// Trapezoid prism: a `base` rectangle (x, y size) at `base_center` rising
    /// `height` to a `top` rectangle offset by `top_shift`. Covers tapered
    /// boxes, wedges (`top.x` near zero) and pyramids.
    pub fn frustum(
        &mut self,
        base_center: Vec3,
        base: Vec2,
        top: Vec2,
        height: f32,
        top_shift: Vec2,
    ) {
        self.loft(
            &frustum_rings(base_center, base, top, height, top_shift),
            true,
            true,
        );
    }

    /// [`Self::frustum`] without its bottom face.
    pub fn frustum_open(
        &mut self,
        base_center: Vec3,
        base: Vec2,
        top: Vec2,
        height: f32,
        top_shift: Vec2,
    ) {
        self.loft(
            &frustum_rings(base_center, base, top, height, top_shift),
            false,
            true,
        );
    }

    /// Armour plate lying on a surface: straight sides, bevelled top edges, no
    /// bottom face. `base_center` is the middle of the underside. Below full
    /// detail the bevel is dropped.
    pub fn plate(&mut self, base_center: Vec3, size: Vec2, thickness: f32, bevel: f32) {
        if !self.fine() {
            self.cuboid_open(
                base_center + Vec3::Z * (thickness * 0.5),
                size.extend(thickness),
            );
            return;
        }
        let c = base_center.truncate();
        let bevel = bevel.min(thickness * 0.9).min(size.min_element() * 0.45);
        let rings = [
            rect_ring(c, size * 0.5, base_center.z),
            rect_ring(c, size * 0.5, base_center.z + thickness - bevel),
            rect_ring(
                c,
                size * 0.5 - Vec2::splat(bevel),
                base_center.z + thickness,
            ),
        ];
        self.loft(&rings, false, true);
    }

    /// Box whose four vertical corners are cut at 45 degrees (octagonal plan).
    pub fn chamfered_box(&mut self, center: Vec3, size: Vec3, chamfer: f32) {
        let profile = chamfered_rect(size.truncate() * 0.5, chamfer);
        self.at(center.truncate().extend(0.0), |b| {
            b.extrude_z(&profile, center.z - size.z * 0.5, center.z + size.z * 0.5)
        });
    }

    // ---- round shapes ----------------------------------------------------

    /// Upright n-gon prism, cylinder, cone (`top_radius` 0) or tapered drum.
    /// A flat side faces +x. Radii are circumradii.
    pub fn prism(
        &mut self,
        base_center: Vec3,
        sides: usize,
        base_radius: f32,
        top_radius: f32,
        height: f32,
    ) {
        let ring = |radius: f32, z: f32| ngon_ring(base_center.truncate(), sides, radius, z);
        self.round = true;
        self.loft(
            &[
                ring(base_radius, base_center.z),
                ring(top_radius, base_center.z + height),
            ],
            true,
            true,
        );
    }

    /// Round bar from `a` to `b` with a radius at each end: barrels, struts, limbs, branches.
    pub fn cylinder_between(
        &mut self,
        a: Vec3,
        b: Vec3,
        radius_a: f32,
        radius_b: f32,
        sides: usize,
    ) {
        let Some((side, up)) = bar_frame(a, b) else {
            return;
        };
        let ring = |center: Vec3, radius: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let angle = (i as f32 + 0.5) * TAU / sides as f32;
                    center + (side * angle.cos() + up * angle.sin()) * radius
                })
                .collect()
        };
        self.round = true;
        self.loft(&[ring(a, radius_a), ring(b, radius_b)], true, true);
    }

    /// Rectangular bar from `a` to `b`. Each size is (width, thickness): for a
    /// bar along x that is (y extent, z extent); for an upright bar (y extent,
    /// x extent); for a bar along y (x extent, z extent).
    pub fn beam(&mut self, a: Vec3, b: Vec3, size_a: Vec2, size_b: Vec2) {
        let Some((side, up)) = bar_frame(a, b) else {
            return;
        };
        let ring = |center: Vec3, size: Vec2| -> Vec<Vec3> {
            let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
            vec![
                center - s - u,
                center + s - u,
                center + s + u,
                center - s + u,
            ]
        };
        self.loft(&[ring(a, size_a), ring(b, size_b)], true, true);
    }

    /// Faceted ellipsoid: `rings` latitude bands of `sides` facets.
    pub fn spheroid(&mut self, center: Vec3, radii: Vec3, sides: usize, rings: usize) {
        self.lumpy_spheroid(center, radii, sides, rings, 0.0, 0);
    }

    /// Ellipsoid whose vertices are pushed in and out by up to `roughness`
    /// (fraction of the radius), deterministically from `seed`: rocks, tree crowns.
    pub fn lumpy_spheroid(
        &mut self,
        center: Vec3,
        radii: Vec3,
        sides: usize,
        rings: usize,
        roughness: f32,
        seed: u32,
    ) {
        let rings = rings.max(2);
        let stack: Vec<Vec<Vec3>> = (0..=rings)
            .map(|j| {
                let latitude = -PI * 0.5 + PI * j as f32 / rings as f32;
                (0..sides)
                    .map(|i| {
                        let longitude = (i as f32 + 0.5 * (j % 2) as f32) * TAU / sides as f32;
                        let pole = j == 0 || j == rings;
                        let bump = if pole {
                            1.0
                        } else {
                            1.0 + roughness * (hash_unit(seed, (j * sides + i) as u32) * 2.0 - 1.0)
                        };
                        let dir = Vec3::new(
                            latitude.cos() * longitude.cos(),
                            latitude.cos() * longitude.sin(),
                            latitude.sin(),
                        );
                        center + dir * radii * bump
                    })
                    .collect()
            })
            .collect();
        self.loft(&stack, true, true);
    }

    // ---- extrusions ------------------------------------------------------

    /// Side profile (x, z) extruded across the body from `y0` to `y1`.
    pub fn extrude_y(&mut self, profile: &[[f32; 2]], y0: f32, y1: f32) {
        let ring = |y: f32| {
            profile
                .iter()
                .map(|p| Vec3::new(p[0], y, p[1]))
                .collect::<Vec<_>>()
        };
        self.loft(&[ring(y0), ring(y1)], true, true);
    }

    /// Side profile (x, z) extruded symmetrically to `±half_width`, with the
    /// outer `chamfer` metres of each side drawn in toward the profile's
    /// middle: a hull with bevelled flanks. Below full detail the bevel is
    /// dropped and this is a plain extrusion.
    pub fn extrude_y_chamfered(&mut self, profile: &[[f32; 2]], half_width: f32, chamfer: f32) {
        if !self.fine() {
            self.extrude_y(profile, -half_width, half_width);
            return;
        }
        let (min, max) = profile_bounds(profile);
        let (mid, half) = ((min + max) * 0.5, (max - min) * 0.5);
        let inset = Vec2::new(
            (half.x - chamfer).max(0.0) / half.x.max(1e-6),
            (half.y - chamfer).max(0.0) / half.y.max(1e-6),
        );
        let ring = |y: f32, scale: Vec2| -> Vec<Vec3> {
            profile
                .iter()
                .map(|p| {
                    let q = mid + (Vec2::new(p[0], p[1]) - mid) * scale;
                    Vec3::new(q.x, y, q.y)
                })
                .collect()
        };
        let inner = (half_width - chamfer).max(0.0);
        self.loft(
            &[
                ring(-half_width, inset),
                ring(-inner, Vec2::ONE),
                ring(inner, Vec2::ONE),
                ring(half_width, inset),
            ],
            true,
            true,
        );
    }

    /// Cross-section (y, z) extruded along the body from `x0` to `x1`.
    pub fn extrude_x(&mut self, profile: &[[f32; 2]], x0: f32, x1: f32) {
        let ring = |x: f32| {
            profile
                .iter()
                .map(|p| Vec3::new(x, p[0], p[1]))
                .collect::<Vec<_>>()
        };
        self.loft(&[ring(x0), ring(x1)], true, true);
    }

    /// Plan profile (x, y) extruded upward from `z0` to `z1`.
    pub fn extrude_z(&mut self, profile: &[[f32; 2]], z0: f32, z1: f32) {
        self.loft_z(profile, &[Section::new(z0, 1.0), Section::new(z1, 1.0)]);
    }

    /// Plan profile (x, y) swept through scaled and shifted `sections`:
    /// faceted shells with sloped cheeks, glacis plates and tumblehome.
    pub fn loft_z(&mut self, profile: &[[f32; 2]], sections: &[Section]) {
        let rings: Vec<Vec<Vec3>> = sections
            .iter()
            .map(|s| {
                profile
                    .iter()
                    .map(|p| (Vec2::new(p[0], p[1]) * s.scale + s.shift).extend(s.z))
                    .collect()
            })
            .collect();
        self.loft(&rings, true, true);
    }

    // ---- core ------------------------------------------------------------

    /// Joins consecutive `rings` (all the same length) with quads and
    /// optionally caps the two ends. Rings may be concave and may collapse to
    /// a point. Orientation is fixed up from the solid's signed volume, caps
    /// included even when they are not emitted.
    pub fn loft(&mut self, rings: &[Vec<Vec3>], cap_start: bool, cap_end: bool) {
        let Some(first) = rings.first() else { return };
        let n = first.len();
        assert!(
            rings.len() >= 2 && n >= 3 && rings.iter().all(|r| r.len() == n),
            "loft needs matching rings"
        );
        let rings: Vec<Vec<Vec3>> = rings
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&p| self.transform.transform_point3(p))
                    .collect()
            })
            .collect();

        // A tube's sides are one surface: a frame that runs round it, so detail is
        // fitted to the whole drum and not to each facet. Its caps stay bare.
        let tube = if std::mem::take(&mut self.round) && n >= 6 {
            Tube::around(&rings)
        } else {
            None
        };

        // (emitted, outline) for the two caps, then the side quads.
        let mut faces: Vec<(bool, Vec<Vec3>)> = Vec::with_capacity(n * (rings.len() - 1) + 2);
        faces.push((cap_start, rings[0].iter().rev().copied().collect()));
        faces.push((cap_end, rings[rings.len() - 1].clone()));
        for pair in rings.windows(2) {
            for i in 0..n {
                let j = (i + 1) % n;
                faces.push((true, vec![pair[0][i], pair[0][j], pair[1][j], pair[1][i]]));
            }
        }

        let volume: f32 = faces
            .iter()
            .map(|(_, f)| {
                (1..f.len() - 1)
                    .map(|i| f[0].dot(f[i].cross(f[i + 1])))
                    .sum::<f32>()
            })
            .sum();
        let inside_out = volume < 0.0;
        #[cfg(test)]
        let start = self.mesh.indices.len();
        for (index, (_, mut face)) in faces
            .into_iter()
            .enumerate()
            .filter(|(_, (emitted, _))| *emitted)
        {
            if inside_out {
                face.reverse();
            }
            self.framing = match tube {
                Some(_) if index < 2 => Framing::Bare,
                Some(tube) => Framing::Tube(tube),
                None => Framing::Flat,
            };
            self.emit_face(&face);
        }
        self.framing = Framing::Flat;
        #[cfg(test)]
        if cap_start && cap_end {
            self.solids.push(start..self.mesh.indices.len());
        }
    }

    /// A single one-sided polygon, wound counter-clockwise seen from its
    /// front: stripes and markings laid just above a surface.
    pub fn face(&mut self, points: &[Vec3]) {
        let mut world: Vec<Vec3> = points
            .iter()
            .map(|&p| self.transform.transform_point3(p))
            .collect();
        if self.transform.matrix3.determinant() < 0.0 {
            world.reverse();
        }
        self.emit_face(&world);
    }

    /// A two-sided cutout foliage card showing `region` (`[u0, v0, u1, v1]`, v
    /// down as the atlas is stored: the region's top edge is at `center + up`) of
    /// the leaf atlas that `tag` picks. Geometric normals stay for winding checks;
    /// the foliage shader lights the leaves from the crown instead:
    /// - `surface`: `pattern::NONE`, then `tag` in the random byte: bit 7 set for
    ///   the conifer atlas, bits 0..7 a per-card random.
    /// - `face`: `crown(p)` at each corner (authored space): xyz the outward
    ///   normal of the crown there, w how deep in the crown it is (0 outside and
    ///   lit, 1 in the dark heart of it).
    pub fn leaf_card(&mut self, center: Vec3, right: Vec3, up: Vec3, region: [f32; 4], tag: u32,
        crown: impl Fn(Vec3) -> [f32; 4]) {
        let corners = [center - right - up, center + right - up,
            center + right + up, center - right + up];
        let [u0, v0, u1, v1] = region;
        let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
        let shade = corners.map(|p| {
            let [x, y, z, w] = crown(p);
            let n = self.transform.matrix3.inverse().transpose() * Vec3::new(x, y, z);
            let n = Vec3::from(n).normalize_or(Vec3::Z);
            [n.x, n.y, n.z, w]
        });
        let points = corners.map(|p| self.transform.transform_point3(p));
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize();
        for back in [false, true] {
            let base = self.mesh.vertices.len() as u32;
            for i in 0..4 {
                self.mesh.vertices.push(MeshVertex {
                    pos: points[i].to_array(), normal: (if back { -normal } else { normal }).to_array(),
                    uv: uv[i], material: material::FOLIAGE, part: self.part, rig: self.rig,
                    face: shade[i], surface: pattern::NONE | (tag & 0xFF) << 8,
                });
            }
            let order = if back { [0, 2, 1, 0, 3, 2] } else { [0, 1, 2, 0, 2, 3] };
            self.mesh.indices.extend(order.map(|i| base + i));
        }
    }

    /// Upward-facing rectangle at height `center.z`.
    pub fn decal(&mut self, center: Vec3, size: Vec2) {
        self.face(&rect_ring(center.truncate(), size * 0.5, center.z));
    }

    /// Emits one polygon given in final (transformed) space.
    fn emit_face(&mut self, points: &[Vec3]) {
        let mut ring: Vec<Vec3> = Vec::with_capacity(points.len());
        for &p in points {
            if ring.last().is_none_or(|q| q.distance(p) > WELD_DISTANCE) {
                ring.push(p);
            }
        }
        while ring.len() > 1 && ring[0].distance(ring[ring.len() - 1]) <= WELD_DISTANCE {
            ring.pop();
        }
        match ring.len() {
            0..=2 => {}
            3 => self.emit_triangles(&ring, &[[0, 1, 2]]),
            4 => {
                let (n0, n1) = (
                    triangle_normal(ring[0], ring[1], ring[2]),
                    triangle_normal(ring[0], ring[2], ring[3]),
                );
                let planar = n0.length() < 1e-9
                    || n1.length() < 1e-9
                    || n0.normalize().dot(n1.normalize()) > 0.9999;
                if planar {
                    self.emit_triangles(&ring, &[[0, 1, 2], [0, 2, 3]]);
                } else {
                    // A twisted quad is two flat facets, each with its own normal.
                    self.emit_triangles(&[ring[0], ring[1], ring[2]], &[[0, 1, 2]]);
                    self.emit_triangles(&[ring[0], ring[2], ring[3]], &[[0, 1, 2]]);
                }
            }
            _ => {
                let triangles = triangulate(&ring);
                self.emit_triangles(&ring, &triangles);
            }
        }
    }

    /// Pushes a planar face: shared flat normal, box-projected UVs.
    fn emit_triangles(&mut self, points: &[Vec3], triangles: &[[usize; 3]]) {
        let Some(normal) = newell_normal(points).try_normalize() else {
            return;
        };
        let kept: Vec<&[usize; 3]> = triangles
            .iter()
            .filter(|t| {
                let n = triangle_normal(points[t[0]], points[t[1]], points[t[2]]);
                n.length() * 0.5 >= MIN_TRIANGLE_AREA && n.dot(normal) > 0.0
            })
            .collect();
        if kept.is_empty() {
            return;
        }
        let base = self.mesh.vertices.len() as u32;
        let abs = normal.abs();
        let frame = match self.framing {
            Framing::Flat => FaceFrame::flat(points, normal),
            Framing::Tube(tube) => Some(tube.frame(points)),
            Framing::Bare => None,
        };
        let seed = frame.as_ref().map_or(0, FaceFrame::seed);
        let surface = self.pattern | seed << 8;
        for &p in points {
            let uv = if abs.x >= abs.y && abs.x >= abs.z {
                [p.y, p.z]
            } else if abs.y >= abs.z {
                [p.x, p.z]
            } else {
                [p.x, p.y]
            };
            self.mesh.vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: normal.to_array(),
                uv,
                material: self.material,
                part: self.part,
                rig: self.rig,
                face: frame.as_ref().map_or([0.0; 4], |f| f.at(p)),
                surface,
            });
        }
        for t in kept {
            self.mesh.indices.extend(t.iter().map(|&i| base + i as u32));
        }
    }

    #[cfg(test)]
    pub fn solids(&self) -> &[std::ops::Range<usize>] {
        &self.solids
    }

    #[cfg(test)]
    pub fn mesh(&self) -> &MeshLod {
        &self.mesh
    }
}

// ---- face frames -----------------------------------------------------------

/// How a face's vertices get their [`MeshVertex::face`] frame.
#[derive(Clone, Copy)]
enum Framing {
    /// From the face's own outline.
    Flat,
    /// From the tube the face is a facet of.
    Tube(Tube),
    /// None: the shader draws no fitted detail on it.
    Bare,
}

/// A face's own coordinate system: the smallest rectangle round its outline.
struct FaceFrame {
    s: Vec3,
    t: Vec3,
    /// Middle of the rectangle, in (s, t).
    centre: Vec2,
    half: Vec2,
    /// A tube: s is the way round, and `centre.x` the face's own angle.
    tube: Option<Tube>,
}

impl FaceFrame {
    /// The frame of a planar polygon, or none for one that fills too little of
    /// its rectangle for an outline along the rectangle to mean anything.
    fn flat(points: &[Vec3], normal: Vec3) -> Option<FaceFrame> {
        let extent = |u: Vec3, v: Vec3| {
            points.iter().fold(
                (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
                |(lo, hi), p| {
                    let q = Vec2::new(p.dot(u), p.dot(v));
                    (lo.min(q), hi.max(q))
                },
            )
        };
        // Level first: on a wall the frame that runs level, on a deck the one square to
        // the model. Lines drawn "horizontal" then are, even on a gable whose longest
        // edge slopes. A tilted frame has to be a much better fit to win (a raked beam).
        let level = if normal.z.abs() < 0.9 {
            Vec3::Z.cross(normal).normalize()
        } else {
            normal.cross(Vec3::X.cross(normal)).normalize()
        };
        let (lo, hi) = extent(level, normal.cross(level));
        let level_area = (hi.x - lo.x) * (hi.y - lo.y);
        let mut best: Option<(f32, Vec3, Vec3)> = Some((level_area * 0.7, level, normal.cross(level)));
        // The smallest bounding rectangle of a convex outline lies along one of its edges.
        for (i, &p) in points.iter().enumerate() {
            let Some(u) = (points[(i + 1) % points.len()] - p).try_normalize() else {
                continue;
            };
            let v = normal.cross(u);
            let (lo, hi) = extent(u, v);
            let area = (hi.x - lo.x) * (hi.y - lo.y);
            // Strictly smaller by a margin, so a rectangle keeps its first edge and mirrored halves agree.
            if best.is_none_or(|(a, ..)| area < a * 0.999) {
                best = Some((area, u, v));
            }
        }
        let (_, a, b) = best?;
        let (lo, hi) = extent(a, b);
        let area = (hi.x - lo.x) * (hi.y - lo.y);
        let covered = newell_normal(points).length() * 0.5;
        if area <= 1e-8 || covered < area * 0.62 {
            return None;
        }
        // On a wall t runs up it, so "horizontal" means the same thing on every
        // face; on a deck s runs the way the model faces.
        let (s, t) = if normal.z.abs() < 0.9 {
            let t = if a.z.abs() >= b.z.abs() { a } else { b };
            let t = if t.z < 0.0 { -t } else { t };
            (t.cross(normal), t)
        } else {
            let s = if a.x.abs() >= b.x.abs() { a } else { b };
            let s = if s.x < 0.0 { -s } else { s };
            (s, normal.cross(s))
        };
        let (lo, hi) = extent(s, t);
        Some(FaceFrame {
            s,
            t,
            centre: (lo + hi) * 0.5,
            half: (hi - lo) * 0.5,
            tube: None,
        })
    }

    fn at(&self, p: Vec3) -> [f32; 4] {
        if let Some(tube) = self.tube {
            let around = tube.angle(p) - self.centre.x;
            let around = self.centre.x + (around + PI).rem_euclid(TAU) - PI;
            return [
                around * tube.radius,
                (p - tube.origin).dot(tube.axis) - tube.length * 0.5,
                -self.half.x,
                self.half.y,
            ];
        }
        [
            p.dot(self.s) - self.centre.x,
            p.dot(self.t) - self.centre.y,
            self.half.x,
            self.half.y,
        ]
    }

    /// A byte of randomness that a face keeps across levels of detail and
    /// shares with its mirror image.
    fn seed(&self) -> u32 {
        let q = |v: f32| (v * 8.0).round() as i32 as u32;
        let middle = match self.tube {
            Some(tube) => tube.origin,
            None => self.s * self.centre.x + self.t * self.centre.y,
        };
        let mut h = 0x9E37_79B9u32;
        for v in [q(middle.x), q(middle.y.abs()), q(self.half.x), q(self.half.y)] {
            h = (h ^ v).wrapping_mul(0x85EB_CA6B);
            h ^= h >> 13;
        }
        (h >> 8) & 0xFF
    }
}

/// The shared frame of a tube's side facets.
#[derive(Clone, Copy)]
struct Tube {
    origin: Vec3,
    axis: Vec3,
    /// Where the angle round the axis is zero.
    zero: Vec3,
    radius: f32,
    length: f32,
}

impl Tube {
    /// From a loft's rings (already in final space), or none for one with no length.
    fn around(rings: &[Vec<Vec3>]) -> Option<Tube> {
        let middle = |ring: &Vec<Vec3>| ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
        let (origin, end) = (middle(&rings[0]), middle(&rings[rings.len() - 1]));
        let length = origin.distance(end);
        let axis = (end - origin).try_normalize()?;
        let mut radius = 0.0;
        for ring in rings {
            let c = middle(ring);
            radius += ring.iter().map(|p| p.distance(c)).sum::<f32>() / ring.len() as f32;
        }
        let radius = radius / rings.len() as f32;
        let reference = if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
        let zero = (reference - axis * reference.dot(axis)).try_normalize()?;
        (radius > 1e-4).then_some(Tube {
            origin,
            axis,
            zero,
            radius,
            length,
        })
    }

    fn angle(&self, p: Vec3) -> f32 {
        let r = p - self.origin;
        r.dot(self.axis.cross(self.zero)).atan2(r.dot(self.zero))
    }

    fn frame(self, points: &[Vec3]) -> FaceFrame {
        let middle = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        FaceFrame {
            s: Vec3::ZERO,
            t: self.axis,
            centre: Vec2::new(self.angle(middle), 0.0),
            half: Vec2::new(PI * self.radius, self.length * 0.5),
            tube: Some(self),
        }
    }
}

// ---- profile helpers -------------------------------------------------------

/// Rectangle plan (x, y) with its corners cut by `chamfer`.
pub fn chamfered_rect(half: Vec2, chamfer: f32) -> Vec<[f32; 2]> {
    let c = chamfer.min(half.min_element() * 0.95);
    let (x, y) = (half.x, half.y);
    vec![
        [x, -y + c],
        [x, y - c],
        [x - c, y],
        [-x + c, y],
        [-x, y - c],
        [-x, -y + c],
        [-x + c, -y],
        [x - c, -y],
    ]
}

/// Regular n-gon plan (x, y) with a flat side facing +x.
pub fn ngon(sides: usize, radius: f32) -> Vec<[f32; 2]> {
    (0..sides)
        .map(|i| {
            let angle = (i as f32 + 0.5) * TAU / sides as f32;
            [angle.cos() * radius, angle.sin() * radius]
        })
        .collect()
}

/// Deterministic hash of (`seed`, `index`) to `0.0..1.0`.
pub fn hash_unit(seed: u32, index: u32) -> f32 {
    let mut h =
        seed.wrapping_mul(0x9E37_79B9) ^ index.wrapping_mul(0x85EB_CA6B).wrapping_add(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn frustum_rings(
    base_center: Vec3,
    base: Vec2,
    top: Vec2,
    height: f32,
    top_shift: Vec2,
) -> [Vec<Vec3>; 2] {
    let c = base_center.truncate();
    [
        rect_ring(c, base * 0.5, base_center.z),
        rect_ring(c + top_shift, top * 0.5, base_center.z + height),
    ]
}

/// Cross-section axes for a bar from `a` to `b`: `side` stays as close to +y
/// (left) as the bar's direction allows, `up` completes the frame.
fn bar_frame(a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let axis = (b - a).try_normalize()?;
    let reference = if axis.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let side = (reference - axis * reference.dot(axis)).normalize();
    Some((side, axis.cross(side)))
}

fn rect_ring(center: Vec2, half: Vec2, z: f32) -> Vec<Vec3> {
    vec![
        Vec3::new(center.x - half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y - half.y, z),
        Vec3::new(center.x + half.x, center.y + half.y, z),
        Vec3::new(center.x - half.x, center.y + half.y, z),
    ]
}

fn ngon_ring(center: Vec2, sides: usize, radius: f32, z: f32) -> Vec<Vec3> {
    ngon(sides, radius)
        .iter()
        .map(|p| Vec3::new(center.x + p[0], center.y + p[1], z))
        .collect()
}

fn profile_bounds(profile: &[[f32; 2]]) -> (Vec2, Vec2) {
    profile.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), p| {
            let v = Vec2::new(p[0], p[1]);
            (lo.min(v), hi.max(v))
        },
    )
}

fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (b - a).cross(c - a)
}

/// Area-weighted polygon normal; robust for concave and slightly non-planar rings.
fn newell_normal(points: &[Vec3]) -> Vec3 {
    let mut normal = Vec3::ZERO;
    for (i, &p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        normal += Vec3::new(
            (p.y - q.y) * (p.z + q.z),
            (p.z - q.z) * (p.x + q.x),
            (p.x - q.x) * (p.y + q.y),
        );
    }
    normal
}

/// Ear-clipping triangulation of a planar, possibly concave polygon.
fn triangulate(points: &[Vec3]) -> Vec<[usize; 3]> {
    let Some(normal) = newell_normal(points).try_normalize() else {
        return Vec::new();
    };
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let flat: Vec<Vec2> = points
        .iter()
        .map(|p| Vec2::new(p.dot(u), p.dot(v)))
        .collect();
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);

    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len() - 2);
    while remaining.len() > 3 {
        let m = remaining.len();
        let ear = (0..m).find(|&k| {
            let (a, b, c) = (
                flat[remaining[(k + m - 1) % m]],
                flat[remaining[k]],
                flat[remaining[(k + 1) % m]],
            );
            if cross(a, b, c) <= 1e-9 {
                return false;
            }
            !remaining.iter().any(|&other| {
                let p = flat[other];
                let corner = p.distance_squared(a) < 1e-10
                    || p.distance_squared(b) < 1e-10
                    || p.distance_squared(c) < 1e-10;
                !corner && cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
            })
        });
        // No ear means leftover collinear points: drop one and carry on.
        let k = ear.unwrap_or(0);
        if ear.is_some() {
            triangles.push([
                remaining[(k + m - 1) % m],
                remaining[k],
                remaining[(k + 1) % m],
            ]);
        }
        remaining.remove(k);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_volume(mesh: &MeshLod, range: std::ops::Range<usize>) -> f32 {
        mesh.indices[range]
            .chunks(3)
            .map(|t| {
                let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
                p(t[0]).dot(p(t[1]).cross(p(t[2]))) / 6.0
            })
            .sum()
    }

    fn volume_of(build: impl Fn(&mut MeshBuilder), transform: Affine3A) -> f32 {
        let mut b = MeshBuilder::new(0, transform);
        build(&mut b);
        let mesh = b.finish();
        let n = mesh.indices.len();
        signed_volume(&mesh, 0..n)
    }

    #[test]
    fn primitives_have_expected_outward_volume() {
        let transforms = [
            Affine3A::IDENTITY,
            Affine3A::from_translation(Vec3::new(30.0, -20.0, 5.0))
                * Affine3A::from_rotation_y(0.7)
                * Affine3A::from_rotation_z(2.0),
            Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)),
            Affine3A::from_translation(Vec3::new(-9.0, 4.0, 1.0))
                * Affine3A::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
        ];
        let l_profile = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        let clockwise: Vec<[f32; 2]> = l_profile.iter().rev().copied().collect();
        type Case<'a> = (&'a str, Box<dyn Fn(&mut MeshBuilder) + 'a>, f32);
        let cases: Vec<Case> = vec![
            (
                "cuboid",
                Box::new(|b| b.cuboid(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 3.0, 4.0))),
                24.0,
            ),
            (
                "frustum",
                Box::new(|b| {
                    b.frustum(
                        Vec3::ZERO,
                        Vec2::new(2.0, 2.0),
                        Vec2::ZERO,
                        3.0,
                        Vec2::new(0.5, 0.0),
                    )
                }),
                4.0,
            ),
            (
                "prism",
                Box::new(|b| b.prism(Vec3::ZERO, 4, 2.0_f32.sqrt(), 2.0_f32.sqrt(), 5.0)),
                20.0,
            ),
            (
                "bar",
                Box::new(|b| {
                    b.cylinder_between(
                        Vec3::ZERO,
                        Vec3::new(3.0, 4.0, 0.0),
                        2.0_f32.sqrt(),
                        2.0_f32.sqrt(),
                        4,
                    )
                }),
                20.0,
            ),
            (
                "concave",
                Box::new(|b| b.extrude_y(&l_profile, -1.0, 1.0)),
                6.0,
            ),
            (
                "clockwise",
                Box::new(|b| b.extrude_z(&clockwise, 0.0, 2.0)),
                6.0,
            ),
            (
                "chamfered",
                Box::new(|b| b.chamfered_box(Vec3::ZERO, Vec3::new(4.0, 4.0, 1.0), 1.0)),
                14.0,
            ),
        ];
        for (name, build, expected) in &cases {
            for transform in transforms {
                let volume = volume_of(build, transform);
                assert!(
                    (volume - expected).abs() < 1e-3 * expected.max(1.0) + 2e-3,
                    "{name}: volume {volume}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn open_primitives_face_outward() {
        let mut b = MeshBuilder::new(0, Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)));
        b.plate(Vec3::new(5.0, 5.0, 1.0), Vec2::new(2.0, 1.0), 0.2, 0.05);
        b.cuboid_open(Vec3::new(-4.0, 3.0, 2.0), Vec3::ONE);
        b.decal(Vec3::new(0.0, 0.0, 1.0), Vec2::ONE);
        let mesh = b.finish();
        assert!(
            mesh.vertices.iter().all(|v| v.normal[2] > -1e-6),
            "no downward faces on open-bottom shapes"
        );
        assert!(mesh.vertices.iter().any(|v| v.normal[2] > 0.99));
        for t in mesh.indices.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
            let n = triangle_normal(p(t[0]), p(t[1]), p(t[2])).normalize();
            assert!(n.dot(Vec3::from(mesh.vertices[t[0] as usize].normal)) > 0.999);
        }
    }

    #[test]
    fn collapsed_rings_make_clean_cones() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.prism(Vec3::ZERO, 6, 1.0, 0.0, 2.0);
        let mesh = b.finish();
        // Six side triangles and a hexagonal base.
        assert_eq!(mesh.indices.len() / 3, 6 + 4);
    }

    #[test]
    fn brush_state_is_scoped() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.paint(material::GLOW);
        b.with_part(part::TURRET, |b| {
            b.at(Vec3::X * 10.0, |b| b.cuboid(Vec3::ZERO, Vec3::ONE))
        });
        b.cuboid(Vec3::ZERO, Vec3::ONE);
        let mesh = b.finish();
        let (turret, hull): (Vec<&MeshVertex>, Vec<&MeshVertex>) =
            mesh.vertices.iter().partition(|v| v.part == part::TURRET);
        assert!(turret
            .iter()
            .all(|v| v.pos[0] > 9.0 && v.material == material::GLOW));
        assert!(hull.iter().all(|v| v.pos[0] < 1.0 && v.part == part::HULL));
        assert_eq!(turret.len(), hull.len());
    }

    /// The shader fits outlines, plates and rivets to `face`: it has to be the
    /// face's real size, with the vertices on its real edges.
    #[test]
    fn a_box_face_is_framed_by_its_own_edges() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.cuboid(Vec3::new(3.0, -2.0, 5.0), Vec3::new(8.0, 4.0, 2.0));
        let mesh = b.finish();
        for v in &mesh.vertices {
            let [s, t, hw, hh] = v.face;
            assert!((s.abs() - hw).abs() < 1e-4 && (t.abs() - hh).abs() < 1e-4, "{v:?}");
            let want = if v.normal[2].abs() > 0.5 {
                [4.0, 2.0]
            } else if v.normal[0].abs() > 0.5 {
                [2.0, 1.0]
            } else {
                [4.0, 1.0]
            };
            assert!((hw - want[0]).abs() < 1e-4 && (hh - want[1]).abs() < 1e-4, "{v:?}");
        }
        // On a wall t runs up it; on a deck s runs the way the model faces.
        for pair in mesh.vertices.chunks(4) {
            let (a, c) = (pair[0], pair[2]);
            let (dt, ds) = (c.face[1] - a.face[1], c.face[0] - a.face[0]);
            if a.normal[2].abs() < 0.5 {
                assert!((dt - (c.pos[2] - a.pos[2])).abs() < 1e-4);
            } else {
                assert!((ds - (c.pos[0] - a.pos[0])).abs() < 1e-4);
            }
        }
    }

    /// A gable's longest edges slope; lines drawn level on it must still be level.
    #[test]
    fn a_gable_keeps_a_level_frame() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.extrude_y(
            &[[-10.0, 0.0], [10.0, 0.0], [10.0, 4.0], [2.0, 9.0], [-6.0, 9.0], [-10.0, 5.0]],
            -3.0,
            3.0,
        );
        let mesh = b.finish();
        let gable: Vec<_> = mesh.vertices.iter().filter(|v| v.normal[1].abs() > 0.9).collect();
        assert_eq!(gable.len(), 12);
        for v in gable {
            assert!((v.face[1] - (v.pos[2] - 4.5)).abs() < 1e-4, "{v:?}");
            assert!((v.face[3] - 4.5).abs() < 1e-4 && (v.face[2] - 10.0).abs() < 1e-4);
        }
    }

    /// A raked bar is framed along itself, not by the big level box round it.
    #[test]
    fn a_raked_beam_is_framed_along_itself() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.beam(Vec3::ZERO, Vec3::new(10.0, 0.0, 10.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0));
        let mesh = b.finish();
        let side: Vec<_> = mesh.vertices.iter().filter(|v| v.normal[1].abs() > 0.9).collect();
        assert!(!side.is_empty());
        for v in side {
            let (long, short) = (v.face[2].max(v.face[3]), v.face[2].min(v.face[3]));
            assert!((long - 50f32.sqrt()).abs() < 1e-3 && (short - 0.5).abs() < 1e-3, "{v:?}");
        }
    }

    /// A tube's facets share one frame that goes right round it; its caps carry none.
    #[test]
    fn a_tube_is_framed_right_round() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.cylinder_between(Vec3::ZERO, Vec3::Z * 6.0, 2.0, 2.0, 12);
        let mesh = b.finish();
        let (caps, sides): (Vec<&MeshVertex>, Vec<&MeshVertex>) =
            mesh.vertices.iter().partition(|v| v.normal[2].abs() > 0.9);
        assert!(caps.iter().all(|v| v.face == [0.0; 4]));
        assert_eq!(sides.len(), 48);
        for v in &sides {
            assert!((v.face[2] + PI * 2.0).abs() < 1e-3, "a negative half width marks the wrap");
            assert!((v.face[3] - 3.0).abs() < 1e-4 && (v.face[1].abs() - 3.0).abs() < 1e-4);
        }
        // Each facet spans a twelfth of the way round, the one over the seam included.
        for facet in sides.chunks(4) {
            let (lo, hi) = facet.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v.face[0]), hi.max(v.face[0]))
            });
            assert!((hi - lo - TAU * 2.0 / 12.0).abs() < 1e-3, "{lo} {hi}");
        }
        // A four-sided prism is a box: flat faces, each with its own outline.
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.prism(Vec3::ZERO, 4, 2.0, 2.0, 3.0);
        assert!(b.finish().vertices.iter().all(|v| v.face[2] > 0.0));
    }

    #[test]
    fn patterns_last_until_the_next_paint_and_mirrors_share_a_seed() {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.paint(material::ACCENT).pattern(pattern::DECK);
        b.cuboid(Vec3::ZERO, Vec3::ONE);
        b.paint(material::ACCENT);
        b.mirror_y(|b| b.cuboid(Vec3::new(0.0, 5.0, 0.0), Vec3::new(3.0, 2.0, 1.0)));
        let mesh = b.finish();
        assert!(mesh.vertices[..24].iter().all(|v| v.surface & 0xFF == pattern::DECK));
        let rest = &mesh.vertices[24..];
        assert!(rest.iter().all(|v| v.surface & 0xFF == pattern::GENERIC));
        let top_seed = |left: bool| {
            rest.iter()
                .find(|v| v.normal[2] > 0.9 && (v.pos[1] > 0.0) == left)
                .map(|v| v.surface >> 8)
        };
        assert_eq!(top_seed(true), top_seed(false));
    }
}
