//! Mesh-building toolkit for the procedural models.
//!
//! A [`MeshBuilder`] is a brush: it carries a current material, part and
//! transform, and every primitive is emitted with them. Each face owns its
//! vertices and gets box-projected UVs in metres. Faces are flat shaded,
//! which suits the hard-edged look, except where a solid is round: there the
//! facets share their normals across the soft edges ([`Smoothing`]), so a
//! drum or a helmet shades as a curve and not as a ring of panels.
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
/// Facets of a solid made round on purpose (a prism, a bar, a sphere) meeting
/// at up to this angle are shaded as one curve: an octagon is round, a
/// hexagon or a box keeps its edges.
const ROUND_CREASE: f32 = 50.0;
/// Any other loft is smoothed only where it bends by up to this much at each
/// of several edges in a row: a run of shallow turns is a curve drawn in
/// facets, a single shallow turn is a crease someone meant.
const CURVE_CREASE: f32 = 35.0;
/// Edges bent less than this are flat anyway: smooth, but not a curve.
const FLAT_CREASE: f32 = 2.0;
/// A shading normal never leans further than this off its own triangle (cosine):
/// where a twisted or folded facet would, it is shaded flat instead.
const MOST_LEAN: f32 = 0.64;

/// The layout of a block of missile cells (`MeshBuilder::cell_block`): `nx` cells along x
/// by `ny` along y, `pitch` apart about `centre`, each hatch `half` wide each way of its
/// cell's centre and shut on `deck`, hinged on its outer edge along y when `hinge_y`, else
/// along x.
#[derive(Clone, Copy, Debug)]
pub struct CellGrid {
    pub centre: Vec2,
    pub deck: f32,
    pub pitch: f32,
    pub half: f32,
    pub nx: u8,
    pub ny: u8,
    pub hinge_y: bool,
}

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
    /// Edges of the solids built now are bevelled this far (m), at full detail ([`Self::with_bevel`]).
    bevel: f32,
    /// The bevel rounds the ends into their caps too, not only the profile's corners.
    bevel_ends: bool,
    /// Solids built now shade every facet flat, however shallow its turns ([`Self::with_facets`]).
    faceted: bool,
    /// How the face being emitted gets its [`MeshVertex::face`] frame.
    framing: Framing,
    transform: Affine3A,
    turret_pivot: Vec3,
    spinner_pivot: Vec3,
    spinner_scans: bool,
    treads: Option<Treads>,
    legs: Option<Legs>,
    hover: bool,
    arm_pivot: Option<[f32; 3]>,
    arm_boom: bool,
    recoil: Option<[f32; 4]>,
    fold: Option<[f32; 4]>,
    breech: Option<[f32; 4]>,
    fold_wrist: Option<[f32; 4]>,
    neck: Option<[f32; 2]>,
    shield_emitter: Option<[f32; 3]>,
    mount: Option<[f32; 4]>,
    houses: Vec<super::House>,
    cells: Vec<super::CellBlock>,
    spins: Vec<(u32, u32, [f32; 3])>,
    pit: Option<super::Pit>,
    excavation: Option<super::Excavation>,
    exhausts: Vec<super::Exhaust>,
    lifts: Vec<super::Lift>,
    vtol: Option<super::Vtol>,
    dust_line: Option<f32>,
    /// The pattern byte leaf cards carry: which leaf atlas the shader samples
    /// ([`Self::leaf_atlas`]).
    leaf_atlas: u32,
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
        Self::at_lod(lod, root)
    }

    /// A builder for the far level past the coarse one ([`Self::far`]): props only a
    /// few pixels across. It is coarse too, so a model with nothing to leave out
    /// there draws its coarse level.
    pub fn new_far(root: Affine3A) -> Self {
        Self::at_lod(LOD_COUNT, root)
    }

    fn at_lod(lod: usize, root: Affine3A) -> Self {
        MeshBuilder {
            lod,
            mesh: MeshLod::default(),
            material: material::PLATING,
            pattern: pattern::GENERIC,
            part: part::HULL,
            rig: 0,
            round: false,
            bevel: 0.0,
            bevel_ends: true,
            faceted: false,
            framing: Framing::Flat,
            transform: root,
            turret_pivot: root.transform_point3(Vec3::ZERO),
            spinner_pivot: root.transform_point3(Vec3::ZERO),
            spinner_scans: false,
            treads: None,
            legs: None,
            hover: false,
            arm_pivot: None,
            arm_boom: false,
            recoil: None,
            fold: None,
            breech: None,
            fold_wrist: None,
            neck: None,
            shield_emitter: None,
            mount: None,
            houses: Vec::new(),
            cells: Vec::new(),
            spins: Vec::new(),
            pit: None,
            excavation: None,
            exhausts: Vec::new(),
            lifts: Vec::new(),
            vtol: None,
            dust_line: None,
            leaf_atlas: pattern::NONE,
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

    /// The last level: a handful of boxes (and the far level past it).
    pub fn coarse(&self) -> bool {
        self.lod >= LOD_COUNT - 1
    }

    /// The far level ([`Self::new_far`]): a prop a few pixels across, where only
    /// its outline and colour are left to see.
    pub fn far(&self) -> bool {
        self.lod == LOD_COUNT
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
    /// Runs `f` with the edges of every solid it builds bevelled `radius`
    /// metres: sharp corners cut and each capped end rounded into its cap, the
    /// cut shaded as a rounded edge. Full detail only; a cut never takes more
    /// than a share of a short edge.
    pub fn with_bevel(&mut self, radius: f32, f: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.bevel, radius);
        f(self);
        self.bevel = previous;
    }

    /// Runs `f` with every solid it builds shaded flat, facet by facet: no run of shallow
    /// turns is taken for a curve and no round solid is smoothed, so hard-edged armour
    /// drawn in many facets reads as flat planes meeting at clear edges.
    pub fn with_facets(&mut self, f: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.faceted, true);
        f(self);
        self.faceted = previous;
    }

    /// [`Self::with_bevel`] for the profile only: the corners a loft's rings
    /// turn are rounded along its length, and its end faces keep sharp rims.
    /// A tread's run bends round at nose and tail; its flat sides stay flat.
    pub fn with_profile_bevel(&mut self, radius: f32, f: impl FnOnce(&mut Self)) {
        let previous = std::mem::replace(&mut self.bevel_ends, false);
        self.with_bevel(radius, f);
        self.bevel_ends = previous;
    }

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

    /// Runs `f` as a ground stake planted when the unit deploys (`gpu_consts::stake`): its
    /// launcher tube, or with `spike` the spike it fires.
    pub fn with_stake(&mut self, spike: bool, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig |= rig::DEPLOY | rig::STAKE | if spike { rig::STAKE_SPIKE } else { 0 };
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
        self.modules
            .iter()
            .position(|k| k == key)
            .map(|i| i as u32 + 1)
    }

    /// Runs `f` as pieces of refit module `key`: drawn only on a unit that has it
    /// fitted (or a later tier over it), and raised `at` (zero to one) of the way
    /// through the refit that fits it. Nothing is emitted when the unit has no such module.
    pub fn module(&mut self, key: &str, at: f32, f: impl FnOnce(&mut Self)) {
        let Some(tag) = self.module_tag(key) else {
            return;
        };
        let previous = self.rig;
        // On a tail or a pincer (`rig::TAIL`) the low four `UPGRADE_AT` bits say which
        // segment it rides, so the time goes up in the top four.
        let when = if previous & rig::LIMB_MASK == rig::TAIL {
            (previous & rig::TAIL_SEG_MASK)
                | ((at.clamp(0.0, 1.0) * 15.0) as u32) << (rig::TAIL_SEG_SHIFT + 4)
        } else {
            ((at.clamp(0.0, 1.0) * 255.0) as u32) << rig::UPGRADE_AT_SHIFT
        };
        self.rig = (previous & !(rig::MODULE_MASK | rig::UPGRADE_AT_MASK))
            | tag << rig::MODULE_SHIFT
            | when;
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

    /// Runs `f` as a breech door hinged along y at `hinge` (current frame), swinging `open`
    /// radians about it after each shot (negative: its bottom goes back and up). Call it
    /// inside the gun's `ARM_GUN` limb: the door rides the gun.
    pub fn with_breech(&mut self, hinge: Vec3, open: f32, f: impl FnOnce(&mut Self)) {
        let at = self.transform.transform_point3(hinge);
        self.breech = Some([at.x, at.y, at.z, open]);
        let previous = self.rig;
        self.rig |= rig::BREECH;
        f(self);
        self.rig = previous;
    }

    pub fn breech(&self) -> Option<[f32; 4]> {
        self.breech
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

    /// Marks where the model's personal (hull) shield is thrown from (current frame):
    /// the field unfolds and its rings run out from here (`Model::shield_emitter`).
    pub fn set_shield_emitter(&mut self, at: Vec3) {
        self.shield_emitter = Some(self.transform.transform_point3(at).to_array());
    }

    /// Marks an engine exhaust port (current frame): the middle of its mouth, which way the
    /// gas leaves it and the mouth's radius. Hot air shimmers above it while the engine
    /// runs (`Model::exhausts`, renderer `heat_haze.rs`).
    pub fn add_exhaust(&mut self, at: Vec3, toward: Vec3, radius: f32) {
        let scale = self.transform.transform_vector3(Vec3::X).length();
        self.exhausts.push(super::Exhaust {
            at: self.transform.transform_point3(at).to_array(),
            toward: self
                .transform
                .transform_vector3(toward)
                .normalize_or(Vec3::Z)
                .to_array(),
            radius: radius * scale,
        });
    }

    /// The exhaust ports marked so far (`add_exhaust`).
    pub fn exhausts(&self) -> Vec<super::Exhaust> {
        self.exhausts.clone()
    }

    /// Marks a plasma lift bell (current frame): the middle of its mouth, where the lift
    /// leaves it downward, and the mouth's radius. Red plasma crackles under it while the
    /// craft is up (`Model::lifts`, renderer `lift_fx.rs`).
    pub fn add_lift(&mut self, at: Vec3, radius: f32) {
        let scale = self.transform.transform_vector3(Vec3::X).length();
        self.lifts.push(super::Lift {
            at: self.transform.transform_point3(at).to_array(),
            radius: radius * scale,
        });
    }

    /// The lift bells marked so far (`add_lift`).
    pub fn lifts(&self) -> Vec<super::Lift> {
        self.lifts.clone()
    }

    /// Declares the model's tilting VTOL pods (`Model::vtol`), in the current frame.
    pub fn set_vtol(&mut self, vtol: super::Vtol) {
        let scale = self.transform.transform_vector3(Vec3::X).length();
        let at = |p: [f32; 3]| self.transform.transform_point3(Vec3::from(p)).to_array();
        self.vtol = Some(super::Vtol {
            pivots: [at(vtol.pivots[0]), at(vtol.pivots[1])],
            nozzle: vtol.nozzle.map(|d| d * scale),
            ..vtol
        });
    }

    pub fn vtol(&self) -> Option<super::Vtol> {
        self.vtol
    }

    /// Where the model's personal shield is thrown from, if the model says (`set_shield_emitter`).
    pub fn shield_emitter(&self) -> Option<[f32; 3]> {
        self.shield_emitter
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
    pub fn with_house(
        &mut self,
        weapon: usize,
        pivot: Vec3,
        travel: f32,
        f: impl FnOnce(&mut Self),
    ) {
        let slot = self
            .houses
            .iter()
            .position(|h| {
                h.weapon as usize == weapon
                    && h.pivot == self.transform.transform_point3(pivot).to_array()
            })
            .unwrap_or_else(|| {
                assert!(
                    (self.houses.len() as u32) < rig::HOUSE_COUNT,
                    "too many gun houses"
                );
                let at = self.transform.transform_point3(pivot);
                let side = self.transform.transform_vector3(Vec3::X).length();
                self.houses.push(super::House {
                    pivot: at.to_array(),
                    travel: travel * side,
                    weapon: weapon as u8,
                });
                self.houses.len() - 1
            });
        let previous = self.rig;
        if slot >= 4 {
            assert!(
                previous & (rig::UPGRADE | rig::MODULE_MASK | rig::UNTIL_MASK) == 0,
                "a fifth or later gun house cannot be a refit piece"
            );
        }
        self.rig = (previous & !(rig::LIMB_MASK | rig::HOUSE_HIGH))
            | (rig::HOUSE_FIRST + (slot as u32 & 3))
            | if slot >= 4 { rig::HOUSE_HIGH } else { 0 };
        f(self);
        self.rig = previous;
    }

    /// Declares a block of hatched missile cells (`CellBlock`) laid out by `grid` in the
    /// current frame. `fire` lists the grid cells (`i` along x, `j` along y) in the order
    /// the weapon's muzzles run; the block's missiles follow any declared before it.
    /// Returns the cells' centres in that order, in the current frame, to draw the hatches
    /// (`part::CELL_HATCH`) and rounds (`part::CELL_ROUND`) on.
    pub fn cell_block(&mut self, grid: CellGrid, fire: &[(u8, u8)]) -> Vec<Vec2> {
        let CellGrid {
            centre,
            deck,
            pitch,
            half,
            nx,
            ny,
            hinge_y,
        } = grid;
        let cells = nx as usize * ny as usize;
        assert!(
            self.cells.len() < super::CellBlock::MAX_BLOCKS
                && cells <= super::CellBlock::MAX_CELLS
                && fire.len() == cells,
            "a cell block has 1 to 8 cells, each fired once, and a model at most two blocks"
        );
        let first: usize = self
            .cells
            .iter()
            .map(|b| b.nx as usize * b.ny as usize)
            .sum();
        assert!(first + cells <= 16, "at most 16 cells on a model");
        let mut order = [u8::MAX; super::CellBlock::MAX_CELLS];
        for (k, &(i, j)) in fire.iter().enumerate() {
            order[i as usize * ny as usize + j as usize] = k as u8;
        }
        assert!(
            order[..cells].iter().all(|&k| k != u8::MAX),
            "a cell is never fired"
        );
        let at = self.transform.transform_point3(centre.extend(deck));
        let scale = self.transform.transform_vector3(Vec3::X).length();
        let block = super::CellBlock {
            centre: [at.x, at.y],
            deck: at.z,
            pitch: pitch * scale,
            half: half * scale,
            nx,
            ny,
            hinge_y,
            first: first as u8,
            order,
        };
        self.cells.push(block);
        fire.iter()
            .map(|&(i, j)| {
                let local = super::CellBlock {
                    centre: centre.to_array(),
                    pitch,
                    ..block
                };
                Vec2::from(local.grid_centre(i as usize, j as usize))
            })
            .collect()
    }

    pub fn cells(&self) -> Vec<super::CellBlock> {
        self.cells.clone()
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
    /// wrong for a low hull under a tall mount (`Model::dust_line`). On a structure it
    /// can only lower the footing's dirt: a raised foundation kept clean.
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
            radius: t
                .transform_vector3(Vec3::new(pit.radius, 0.0, 0.0))
                .length(),
            stroke: up(pit.stroke),
            section: up(pit.section),
            rack: [rack.x, rack.y],
            afloat_lift: up(pit.afloat_lift),
        });
    }

    pub fn pit(&self) -> Option<super::Pit> {
        self.pit
    }

    /// Records a mine's excavation beam (given in the current frame).
    pub fn set_excavation(&mut self, beam: super::Excavation) {
        let t = self.transform;
        let at = |p: [f32; 3]| t.transform_point3(Vec3::from(p)).to_array();
        self.excavation = Some(super::Excavation {
            emitter: at(beam.emitter),
            width: t
                .transform_vector3(Vec3::new(beam.width, 0.0, 0.0))
                .length(),
            pinches: beam.pinches.iter().map(|&p| at(p)).collect(),
            surge: beam.surge,
        });
    }

    pub fn excavation(&self) -> Option<super::Excavation> {
        self.excavation.clone()
    }

    /// Records the spinner axis (given in the current frame).
    pub fn set_spinner_pivot(&mut self, pivot: Vec3) {
        self.spinner_pivot = self.transform.transform_point3(pivot);
    }

    /// The spinner looks about instead of turning round and round: it swings slowly
    /// one way and the other, and dwells (a watching eye, not a radar dish).
    pub fn set_spinner_scan(&mut self) {
        self.spinner_scans = true;
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
            sole_chamfer: 0.0,
            hock: None,
            crawl: None,
        });
    }

    /// Makes the leg reverse-kneed (after `set_legs`): the left hock at rest (current
    /// frame), between the knee and the ankle. The thigh (`rig::THIGH`) runs hip to knee,
    /// the shin (`rig::SHIN`) knee to hock and the tarsus (`rig::TARSUS`) hock to ankle; the
    /// tarsus leans with `follow` of the leg's swing and folds as the leg shortens, and the
    /// thigh and shin are solved to meet it.
    pub fn set_hock(&mut self, hock: Vec3, follow: f32) {
        let at = self.transform.transform_point3(hock).to_array();
        self.legs.as_mut().expect("set_legs first").hock = Some((at, follow));
    }

    /// A many-legged walker: the left leg of each pair at rest (hip, knee, foot tip on the
    /// ground, current frame) and where in the cycle it lifts; the right legs are their
    /// mirrors, half a cycle on. The stride, stance and lift are shared, as `set_legs`.
    /// Legs are then emitted `with_pair` and `with_limb(rig::THIGH | SHIN)`.
    pub fn set_crawl_legs(
        &mut self,
        pairs: &[(Vec3, Vec3, Vec3, f32)],
        stride: f32,
        stance: f32,
        lift: f32,
    ) {
        assert!((1..=super::MAX_CRAWL_PAIRS).contains(&pairs.len()));
        let (h, k, a, _) = pairs[0];
        self.set_legs(h, k, a, stride, stance, lift);
        let at = |p: Vec3| self.transform.transform_point3(p).to_array();
        let mut crawl = super::Crawl {
            pairs: pairs.len(),
            joints: [[[0.0; 3]; 3]; super::MAX_CRAWL_PAIRS],
            phase: [0.0; super::MAX_CRAWL_PAIRS],
            tail: [0.0; 2],
            tail_joints: [[0.0; 2]; super::MAX_TAIL_JOINTS],
            tail_count: 0,
            claw: None,
            throws: None,
        };
        for (i, &(hip, knee, ankle, phase)) in pairs.iter().enumerate() {
            crawl.joints[i] = [at(hip), at(knee), at(ankle)];
            crawl.phase[i] = phase.rem_euclid(1.0);
        }
        self.legs.as_mut().unwrap().crawl = Some(crawl);
    }

    /// Runs `f` as leg pair `pair` of a many-legged walker (`set_crawl_legs`).
    pub fn with_pair(&mut self, pair: usize, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig = (previous & !rig::PAIR_MASK) | (pair as u32) << rig::PAIR_SHIFT;
        f(self);
        self.rig = previous;
    }

    /// A many-legged walker's tail: its spine's joints at rest (current frame, in the
    /// y = 0 plane), root first. Segment `i` (`with_tail`) turns about joint `i`; the
    /// turret's own pieces ride the last. The whole tail bends toward the turret's facing
    /// by how high it is, from the root's height to `top`. After `set_crawl_legs`.
    pub fn set_tail(&mut self, joints: &[Vec3], top: f32) {
        assert!((2..=super::MAX_TAIL_JOINTS).contains(&joints.len()));
        let at = |p: Vec3| self.transform.transform_point3(p);
        let z = |h: f32| self.transform.transform_point3(Vec3::Z * h).z;
        let tail = [at(joints[0]).z, z(top)];
        let crawl = self
            .legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first");
        crawl.tail = tail;
        crawl.tail_count = joints.len();
        for (i, &j) in joints.iter().enumerate() {
            let p = self.transform.transform_point3(j);
            crawl.tail_joints[i] = [p.x, p.z];
        }
    }

    /// Runs `f` as segment `segment` of the tail (`set_tail`): turret pieces that bend
    /// about their joint rather than turn.
    pub fn with_tail(&mut self, segment: usize, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        self.rig = (previous & !rig::TAIL_SEG_MASK) | (segment as u32) << rig::TAIL_SEG_SHIFT;
        self.with_part(super::part::TURRET, |b| b.with_limb(rig::TAIL, f));
        self.rig = previous;
    }

    /// A many-legged walker's pincers: the left one's shoulder and its moving finger's
    /// hinge (current frame); the right is the mirror. After `set_crawl_legs`.
    pub fn set_claw(&mut self, shoulder: Vec3, hinge: Vec3) {
        let at = |p: Vec3| self.transform.transform_point3(p).to_array();
        let claw = [at(shoulder), at(hinge)];
        self.legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first")
            .claw = Some(claw);
    }

    /// The weapon slots the left and right pincers throw with (`Crawl::throws`): each
    /// snaps open and kicks on its own weapon's shots.
    pub fn set_claw_throws(&mut self, left: u8, right: u8) {
        assert!(left < 15 && right < 15);
        self.legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first")
            .throws = Some([left, right]);
    }

    /// Runs `f` as a pincer (`set_claw`): its arm, or with `jaw` its moving finger.
    pub fn with_claw(&mut self, jaw: bool, f: impl FnOnce(&mut Self)) {
        let previous = self.rig;
        let seg = if jaw { rig::CLAW_JAW } else { rig::CLAW_ARM };
        self.rig = (previous & !rig::TAIL_SEG_MASK) | seg << rig::TAIL_SEG_SHIFT;
        self.with_limb(rig::TAIL, f);
        self.rig = previous;
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

    /// Records how far back the sole's corners are cut at 45 degrees (after `set_foot`).
    pub fn set_sole_chamfer(&mut self, cut: f32) {
        let side = self.transform.transform_vector3(Vec3::Y).length();
        if let Some(legs) = &mut self.legs {
            legs.sole_chamfer = cut * side;
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

    pub fn spinner_scans(&self) -> bool {
        self.spinner_scans
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

    /// Ellipsoid of `rings` latitude bands of `sides` facets, shaded round.
    pub fn spheroid(&mut self, center: Vec3, radii: Vec3, sides: usize, rings: usize) {
        self.round = true;
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
        // Bevelled where drawn, before the transform: a scaled model scales its bevels too.
        let (rings, bevel) = if self.bevel > 0.0 && self.fine() && bevels_on() {
            let ends = self.bevel_ends;
            let (rings, bevel) = bevel_rings(
                rings.to_vec(),
                self.bevel,
                self.round,
                cap_start && ends,
                cap_end && ends,
            );
            (rings, Some(bevel))
        } else {
            (rings.to_vec(), None)
        };
        let rings: Vec<Vec<Vec3>> = rings
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&p| self.transform.transform_point3(p))
                    .collect()
            })
            .collect();

        let round = std::mem::take(&mut self.round);
        let n = rings[0].len();
        let smoothing = if self.faceted {
            Smoothing::flat(n, rings.len())
        } else {
            Smoothing::of(&rings, round, bevel.as_ref())
        };
        // A tube's sides are one surface: a frame that runs round it, so detail is
        // fitted to the whole drum and not to each facet. Its caps stay bare.
        let tube = if !self.faceted && (round || smoothing.curved) && n >= 6 {
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

        // Signed volume about a corner of the solid itself, so a small piece far from the
        // model's origin (a gun on a 500 m hull) keeps the precision to tell.
        let o = rings[0][0];
        let volume: f32 = faces
            .iter()
            .map(|(_, f)| {
                (1..f.len() - 1)
                    .map(|i| (f[0] - o).dot((f[i] - o).cross(f[i + 1] - o)))
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
            // Caps are flat; a side facet takes its corners' normals from the smoothing.
            let mut normals = (index >= 2)
                .then(|| smoothing.corners((index - 2) / n, (index - 2) % n, inside_out))
                .flatten();
            if inside_out {
                face.reverse();
                if let Some(normals) = normals.as_mut() {
                    normals.reverse();
                }
            }
            self.framing = match tube {
                // A tube's end is a bare disc; a slab found to be curved keeps its deck.
                Some(_) if index < 2 && smoothing.curved => Framing::Flat,
                Some(_) if index < 2 => Framing::Bare,
                Some(tube) => Framing::Tube(tube),
                None => Framing::Flat,
            };
            self.emit_face(&face, normals.as_deref());
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
        self.emit_face(&world, None);
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
    pub fn leaf_card(
        &mut self,
        center: Vec3,
        right: Vec3,
        up: Vec3,
        region: [f32; 4],
        tag: u32,
        crown: impl Fn(Vec3) -> [f32; 4],
    ) {
        let corners = [
            center - right - up,
            center + right - up,
            center + right + up,
            center - right + up,
        ];
        let [u0, v0, u1, v1] = region;
        let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
        let shade = corners.map(|p| {
            let [x, y, z, w] = crown(p);
            let n = self.transform.matrix3.inverse().transpose() * Vec3::new(x, y, z);
            let n = n.normalize_or(Vec3::Z);
            [n.x, n.y, n.z, w]
        });
        let points = corners.map(|p| self.transform.transform_point3(p));
        let normal = (points[1] - points[0])
            .cross(points[2] - points[0])
            .normalize();
        for back in [false, true] {
            let base = self.mesh.vertices.len() as u32;
            for i in 0..4 {
                self.mesh.vertices.push(MeshVertex {
                    pos: points[i].to_array(),
                    normal: (if back { -normal } else { normal }).to_array(),
                    uv: uv[i],
                    material: material::FOLIAGE,
                    part: self.part,
                    rig: self.rig,
                    face: shade[i],
                    surface: self.leaf_atlas | (tag & 0xFF) << 8,
                });
            }
            let order = if back {
                [0, 2, 1, 0, 3, 2]
            } else {
                [0, 1, 2, 0, 2, 3]
            };
            self.mesh.indices.extend(order.map(|i| base + i));
        }
    }

    /// Which leaf atlas the leaf cards that follow show: `pattern::NONE` (the
    /// default) for the temperate atlases their tag picks, `pattern::PLAIN` for
    /// the tropical one (entity.wgsl `LEAF_TROPICAL`).
    pub fn leaf_atlas(&mut self, pattern: u32) -> &mut Self {
        self.leaf_atlas = pattern;
        self
    }

    /// Upward-facing rectangle at height `center.z`.
    pub fn decal(&mut self, center: Vec3, size: Vec2) {
        self.face(&rect_ring(center.truncate(), size * 0.5, center.z));
    }

    /// Emits one polygon given in final (transformed) space, flat shaded
    /// unless `normals` gives each point its own shading normal.
    fn emit_face(&mut self, points: &[Vec3], normals: Option<&[Vec3]>) {
        let mut ring: Vec<Vec3> = Vec::with_capacity(points.len());
        let mut shading: Vec<Vec3> = Vec::with_capacity(points.len());
        for (k, &p) in points.iter().enumerate() {
            if ring.last().is_none_or(|q| q.distance(p) > WELD_DISTANCE) {
                ring.push(p);
                shading.extend(normals.map(|n| n[k]));
            }
        }
        while ring.len() > 1 && ring[0].distance(ring[ring.len() - 1]) <= WELD_DISTANCE {
            ring.pop();
            shading.truncate(ring.len());
        }
        let shading = normals.is_some().then_some(shading.as_slice());
        match ring.len() {
            0..=2 => {}
            3 => self.emit_triangles(&ring, shading, &[[0, 1, 2]]),
            4 => {
                let (n0, n1) = (
                    triangle_normal(ring[0], ring[1], ring[2]),
                    triangle_normal(ring[0], ring[2], ring[3]),
                );
                let planar = n0.length() < 1e-9
                    || n1.length() < 1e-9
                    || n0.normalize().dot(n1.normalize()) > 0.9999;
                if planar {
                    self.emit_triangles(&ring, shading, &[[0, 1, 2], [0, 2, 3]]);
                } else {
                    // A twisted quad is two flat facets, each with its own normal.
                    let pick = |k: [usize; 3]| shading.map(|s| k.map(|k| s[k]));
                    self.emit_triangles(
                        &[ring[0], ring[1], ring[2]],
                        pick([0, 1, 2]).as_ref().map(|s| &s[..]),
                        &[[0, 1, 2]],
                    );
                    self.emit_triangles(
                        &[ring[0], ring[2], ring[3]],
                        pick([0, 2, 3]).as_ref().map(|s| &s[..]),
                        &[[0, 1, 2]],
                    );
                }
            }
            _ => {
                let triangles = triangulate(&ring);
                self.emit_triangles(&ring, shading, &triangles);
            }
        }
    }

    /// Pushes a planar face: box-projected UVs, and the face's own flat normal
    /// or, from a round solid, the `shading` normal at each point.
    fn emit_triangles(
        &mut self,
        points: &[Vec3],
        shading: Option<&[Vec3]>,
        triangles: &[[usize; 3]],
    ) {
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
        for (k, &p) in points.iter().enumerate() {
            let uv = if abs.x >= abs.y && abs.x >= abs.z {
                [p.y, p.z]
            } else if abs.y >= abs.z {
                [p.x, p.z]
            } else {
                [p.x, p.y]
            };
            self.mesh.vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: shading
                    .map_or(normal, |s| {
                        if s[k].dot(normal) >= MOST_LEAN {
                            s[k]
                        } else {
                            normal
                        }
                    })
                    .to_array(),
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

// ---- smooth shading --------------------------------------------------------

/// Which edges of a loft's side facets are soft, and the facets' own normals.
///
/// The side facets form a grid: band `b` joins ring `b` to ring `b + 1`,
/// column `i` joins ring point `i` to `i + 1`. An edge down the loft at ring
/// point `v` is soft when the profile rounds there; an edge round it at ring
/// `r` when the solid does not break there. A corner's normal is the average
/// of the facets meeting there that its facet reaches over soft edges only,
/// so hard edges stay sharp and a curve shades as one surface.
struct Smoothing {
    n: usize,
    /// Each side facet's outward normal and area, by band then column; none if it has no area.
    faces: Vec<Vec<Option<(Vec3, f32)>>>,
    /// The two caps' outward normals and areas, where a bevel rounds into them.
    caps: [Option<(Vec3, f32)>; 2],
    /// Soft edge down the loft at each ring point.
    down: Vec<bool>,
    /// Soft edge round the loft at each ring (the end rings never are).
    round: Vec<bool>,
    /// Most of the way round is a curve: the sides are one surface.
    curved: bool,
}

/// `MERIDIAN_BEVEL=0` leaves every edge sharp again, for before/after shots.
fn bevels_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MERIDIAN_BEVEL").map_or(true, |v| v != "0"))
}

/// `MERIDIAN_SMOOTH=0` shades every facet flat again, for before/after shots.
fn smoothing_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MERIDIAN_SMOOTH").map_or(true, |v| v != "0"))
}

impl Smoothing {
    /// Every facet shaded flat: `n` points round, `len` rings.
    fn flat(n: usize, len: usize) -> Smoothing {
        Smoothing {
            n,
            faces: vec![vec![None; n]; len - 1],
            caps: [None; 2],
            down: vec![false; n],
            round: vec![false; len],
            curved: false,
        }
    }

    fn of(rings: &[Vec<Vec3>], round: bool, bevel: Option<&Bevel>) -> Smoothing {
        let n = rings[0].len();
        if !smoothing_on() {
            return Smoothing::flat(n, rings.len());
        }
        let area_normal = |points: &[Vec3]| {
            let v = newell_normal(points);
            v.try_normalize().map(|unit| (unit, v.length() * 0.5))
        };
        let faces: Vec<Vec<Option<(Vec3, f32)>>> = rings
            .windows(2)
            .map(|pair| {
                (0..n)
                    .map(|i| {
                        let j = (i + 1) % n;
                        area_normal(&[pair[0][i], pair[0][j], pair[1][j], pair[1][i]])
                    })
                    .collect()
            })
            .collect();
        let bend = |a: Option<(Vec3, f32)>, b: Option<(Vec3, f32)>| match (a, b) {
            (Some((a, _)), Some((b, _))) => a.dot(b).clamp(-1.0, 1.0).acos().to_degrees(),
            _ => 0.0,
        };
        // The sharpest the solid bends anywhere along each edge line.
        let down_bend: Vec<f32> = (0..n)
            .map(|v| {
                faces
                    .iter()
                    .map(|band| bend(band[(v + n - 1) % n], band[v]))
                    .fold(0.0, f32::max)
            })
            .collect();
        let round_bend: Vec<f32> = (0..rings.len())
            .map(|r| {
                if r == 0 || r + 1 == rings.len() {
                    return 180.0;
                }
                (0..n)
                    .map(|i| bend(faces[r - 1][i], faces[r][i]))
                    .fold(0.0, f32::max)
            })
            .collect();
        let (mut down, curved_down) = soft_edges(&down_bend, true, round);
        let (mut round_edges, _) = soft_edges(&round_bend, false, round);
        let curved = !round && n >= 6 && curved_down * 2 >= n && ring_is_round(rings);
        // A bevel's facets are one rounded edge, whatever their angles.
        let mut caps = [None; 2];
        if let Some(bevel) = bevel {
            down.iter_mut().zip(&bevel.down).for_each(|(d, b)| *d |= *b);
            round_edges
                .iter_mut()
                .zip(&bevel.round)
                .for_each(|(d, b)| *d |= *b);
            let ends = [
                rings[0].iter().rev().copied().collect::<Vec<_>>(),
                rings[rings.len() - 1].clone(),
            ];
            for (k, end) in ends.iter().enumerate() {
                caps[k] = if bevel.caps[k] {
                    area_normal(end)
                } else {
                    None
                };
            }
        }
        Smoothing {
            n,
            faces,
            caps,
            down,
            round: round_edges,
            curved,
        }
    }

    /// The shading normals at facet (`band`, `i`)'s corners, in the order the
    /// loft lists them: (band, i), (band, i + 1), (band + 1, i + 1), (band + 1, i);
    /// none for a facet shaded flat. Each corner averages the facets it reaches
    /// over soft edges, weighted by area: a broad face keeps its own normal and
    /// a thin bevel beside it turns from one face's normal to the other's.
    fn corners(&self, band: usize, i: usize, inside_out: bool) -> Option<Vec<Vec3>> {
        let n = self.n;
        let flip = if inside_out { -1.0 } else { 1.0 };
        let own = self.faces[band][i]?.0;
        // Only what bends gently away from this facet: never round a fold.
        let gentle = |f: &(Vec3, f32)| f.0.dot(own) >= MOST_LEAN;
        let at = |r: usize, v: usize| -> Vec3 {
            let mut sum = Vec3::ZERO;
            for b in [r.wrapping_sub(1), r] {
                if b >= self.faces.len() || (b != band && !self.round[r]) {
                    continue;
                }
                for c in [(v + n - 1) % n, v] {
                    if c != i && !self.down[v] {
                        continue;
                    }
                    if let Some((f, area)) = self.faces[b][c].filter(gentle) {
                        sum += f * area;
                    }
                }
            }
            let end = if r == 0 {
                self.caps[0]
            } else if r == self.faces.len() {
                self.caps[1]
            } else {
                None
            };
            if let Some((f, area)) = end.filter(gentle) {
                sum += f * area;
            }
            sum.try_normalize().unwrap_or(own) * flip
        };
        let j = (i + 1) % n;
        Some(vec![
            at(band, i),
            at(band, j),
            at(band + 1, j),
            at(band + 1, i),
        ])
    }
}

// ---- bevels ------------------------------------------------------------------

/// Which edges of a bevelled loft's rings are its bevel, so they shade soft.
struct Bevel {
    /// Ring points on a rounded profile corner.
    down: Vec<bool>,
    /// Rings inside a rounded end.
    round: Vec<bool>,
    /// Which ends were rounded into their caps.
    caps: [bool; 2],
}

/// Facets a bevel turning `turn` degrees is cut into: one up to a right angle
/// (the shading rounds it), more for a sharper corner.
fn bevel_steps(turn: f32) -> usize {
    if turn <= 100.0 {
        1
    } else if turn <= 150.0 {
        2
    } else {
        3
    }
}

/// A point on the round from `a` to `b` about corner `p`, `s` from 0 to 1.
fn round_corner(a: Vec3, p: Vec3, b: Vec3, s: f32) -> Vec3 {
    a * ((1.0 - s) * (1.0 - s)) + p * (2.0 * s * (1.0 - s)) + b * (s * s)
}

/// The rings of a loft with its edges bevelled `radius` metres: every sharp
/// profile corner cut the same way in every ring (so the rings still match),
/// and each capped end rounded into its cap by extra rings. A round loft keeps
/// its profile and only has its ends rounded. Radii shrink to fit short edges.
fn bevel_rings(
    rings: Vec<Vec<Vec3>>,
    radius: f32,
    round: bool,
    cap_start: bool,
    cap_end: bool,
) -> (Vec<Vec<Vec3>>, Bevel) {
    let n = rings[0].len();
    let turn_at = |ring: &Vec<Vec3>, i: usize| -> Option<f32> {
        let p = ring[i];
        let a = (ring[(i + n - 1) % n] - p).try_normalize()?;
        let b = (ring[(i + 1) % n] - p).try_normalize()?;
        Some(180.0 - a.dot(b).clamp(-1.0, 1.0).acos().to_degrees())
    };
    let steps: Vec<usize> = (0..n)
        .map(|i| {
            let turn = rings
                .iter()
                .filter_map(|ring| turn_at(ring, i))
                .fold(0.0, f32::max);
            if round || turn <= CURVE_CREASE {
                0
            } else {
                bevel_steps(turn)
            }
        })
        .collect();
    let mut rings: Vec<Vec<Vec3>> = rings
        .iter()
        .map(|ring| {
            let mut out = Vec::with_capacity(n * 2);
            for (i, &p) in ring.iter().enumerate() {
                let k = steps[i];
                if k == 0 {
                    out.push(p);
                    continue;
                }
                let (a, b) = (ring[(i + n - 1) % n], ring[(i + 1) % n]);
                let (u, w) = ((a - p).normalize_or_zero(), (b - p).normalize_or_zero());
                let half_turn = (PI - u.dot(w).clamp(-1.0, 1.0).acos()) * 0.5;
                let reach = (radius * half_turn.tan())
                    .min(0.4 * p.distance(a))
                    .min(0.4 * p.distance(b));
                let (from, to) = (p + u * reach, p + w * reach);
                out.extend((0..=k).map(|j| round_corner(from, p, to, j as f32 / k as f32)));
            }
            out
        })
        .collect();
    let down: Vec<bool> = steps
        .iter()
        .flat_map(|&k| std::iter::repeat_n(k > 0, if k == 0 { 1 } else { k + 1 }))
        .collect();

    // An end rounded into its cap: rings from the cap's edge (first) back to the side (last).
    let round_end = |cap: &Vec<Vec3>, next: &Vec<Vec3>| -> Option<Vec<Vec<Vec3>>> {
        let m = cap.len();
        let middle = |r: &Vec<Vec3>| r.iter().copied().sum::<Vec3>() / r.len() as f32;
        let (c, into) = (middle(cap), middle(next) - middle(cap));
        let winding = newell_normal(cap);
        let mut normal = winding.try_normalize()?;
        if normal.dot(into) < 0.0 {
            normal = -normal;
        }
        let ccw = winding.dot(normal) > 0.0;
        let depth = (0..m)
            .map(|v| cap[v].distance(next[v]))
            .fold(f32::MAX, f32::min);
        let width = cap
            .iter()
            .map(|p| (*p - c).reject_from(normal).length())
            .fold(f32::MAX, f32::min);
        let d = radius.min(0.4 * depth).min(0.35 * width);
        if d < 1e-4 {
            return None;
        }
        let inward = |e: Vec3| {
            if ccw {
                normal.cross(e)
            } else {
                e.cross(normal)
            }
            .normalize_or_zero()
        };
        let mut edge_turn = 0.0f32;
        let ends: Vec<(Vec3, Vec3, Vec3)> = (0..m)
            .map(|v| {
                let p = cap[v];
                let (e0, e1) = (
                    inward(p - cap[(v + m - 1) % m]),
                    inward(cap[(v + 1) % m] - p),
                );
                let dir = (e0 + e1).normalize_or(e0);
                let stretch = 1.0 / dir.dot(e0).max(dir.dot(e1)).max(0.5);
                let side = (next[v] - p).normalize_or(normal);
                edge_turn =
                    edge_turn.max(180.0 - dir.dot(side).clamp(-1.0, 1.0).acos().to_degrees());
                (p + dir * (d * stretch), p, p + side * d)
            })
            .collect();
        let k = bevel_steps(edge_turn);
        Some(
            (0..=k)
                .map(|j| {
                    ends.iter()
                        .map(|&(a, p, b)| round_corner(a, p, b, j as f32 / k as f32))
                        .collect()
                })
                .collect(),
        )
    };
    let mut soft = vec![false; rings.len()];
    let mut caps = [false; 2];
    if cap_start && rings.len() >= 2 {
        if let Some(end) = round_end(&rings[0], &rings[1]) {
            let k = end.len() - 1;
            rings.splice(0..1, end);
            soft.splice(0..1, (0..=k).map(|j| j > 0));
            caps[0] = true;
        }
    }
    if cap_end && rings.len() >= 2 {
        let last = rings.len() - 1;
        if let Some(mut end) = round_end(&rings[last], &rings[last - 1]) {
            end.reverse();
            let k = end.len() - 1;
            rings.splice(last.., end);
            soft.splice(last.., (0..=k).map(|j| j < k));
            caps[1] = true;
        }
    }
    (
        rings,
        Bevel {
            down,
            round: soft,
            caps,
        },
    )
}

/// Which of a line of edges are soft, from how sharply each bends, and how
/// many of them are curve (not merely flat). `cyclic` edges wrap round.
fn soft_edges(bend: &[f32], cyclic: bool, round: bool) -> (Vec<bool>, usize) {
    let len = bend.len();
    if round {
        return (bend.iter().map(|&a| a <= ROUND_CREASE).collect(), 0);
    }
    let shallow: Vec<bool> = bend
        .iter()
        .map(|&a| a > FLAT_CREASE && a <= CURVE_CREASE)
        .collect();
    let at = |k: isize| -> bool {
        if cyclic {
            shallow[k.rem_euclid(len as isize) as usize]
        } else {
            (0..len as isize).contains(&k) && shallow[k as usize]
        }
    };
    // A curve is at least three shallow turns in a row (two along a loft, which has fewer rings).
    let need = if cyclic { 3 } else { 2 };
    let mut soft = vec![false; len];
    let mut curve = 0;
    for k in 0..len {
        let mut run = 1;
        let mut back = k as isize - 1;
        while run < need && at(back) {
            run += 1;
            back -= 1;
        }
        let mut ahead = k as isize + 1;
        while run < need && at(ahead) {
            run += 1;
            ahead += 1;
        }
        let in_curve = shallow[k] && run >= need;
        soft[k] = bend[k] <= FLAT_CREASE || in_curve;
        curve += in_curve as usize;
    }
    (soft, curve)
}

/// A ring near enough a circle for a tube's frame to lay plates on it evenly:
/// its area close to a circle's of the same perimeter. A helmet's ellipse or an
/// octagon is (0.95); a slab with rounded corners is not (0.82-0.88 unless the
/// corners are most of it).
fn ring_is_round(rings: &[Vec<Vec3>]) -> bool {
    rings.iter().any(|ring| {
        let perimeter: f32 = (0..ring.len())
            .map(|i| ring[i].distance(ring[(i + 1) % ring.len()]))
            .sum();
        let area = newell_normal(ring).length() * 0.5;
        perimeter > 1e-3 && 4.0 * PI * area / (perimeter * perimeter) >= 0.93
    })
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
        let mut best: Option<(f32, Vec3, Vec3)> =
            Some((level_area * 0.7, level, normal.cross(level)));
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
        for v in [
            q(middle.x),
            q(middle.y.abs()),
            q(self.half.x),
            q(self.half.y),
        ] {
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

    /// How far the shading normals of a mesh's triangles lean off their faces, at most (degrees).
    fn most_lean(mesh: &MeshLod) -> f32 {
        let mut most = 0.0f32;
        for t in mesh.indices.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
            let n = triangle_normal(p(t[0]), p(t[1]), p(t[2])).normalize();
            for &i in t {
                let dot = n
                    .dot(Vec3::from(mesh.vertices[i as usize].normal))
                    .clamp(-1.0, 1.0);
                most = most.max(dot.acos().to_degrees());
            }
        }
        most
    }

    #[test]
    fn round_solids_shade_smooth_and_boxes_stay_sharp() {
        let build = |f: &dyn Fn(&mut MeshBuilder)| {
            let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
            f(&mut b);
            b.finish()
        };
        // A twelve-sided bar leans half its 30 degree facet angle at the seams.
        let bar = build(&|b| b.cylinder_between(Vec3::ZERO, Vec3::X * 4.0, 1.0, 1.0, 12));
        assert!(
            (most_lean(&bar) - 15.0).abs() < 1.0,
            "bar leans {}",
            most_lean(&bar)
        );
        // Caps stay flat: some normals are the axis exactly.
        assert!(bar.vertices.iter().any(|v| v.normal[0].abs() > 0.9999));
        // (Flat, give or take the rounding in acos near 1.)
        let flat = |mesh: MeshLod| most_lean(&mesh) < 0.1;
        assert!(flat(build(
            &|b| b.cuboid(Vec3::ZERO, Vec3::new(2.0, 3.0, 1.0))
        )));
        assert!(flat(build(&|b| b.chamfered_box(
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 1.0),
            0.4
        ))));
        // A hexagon is a nut, not a drum; a four-sided bar is a box.
        assert!(flat(build(&|b| b.prism(Vec3::ZERO, 6, 1.0, 1.0, 2.0))));
        assert!(flat(build(&|b| b.cylinder_between(
            Vec3::ZERO,
            Vec3::X,
            1.0,
            1.0,
            4
        ))));
        // A loft that only bends once, gently, keeps that crease.
        let wedge = [
            [-2.0, -1.0],
            [2.0, -1.0],
            [2.0, 0.6],
            [0.0, 1.0],
            [-2.0, 0.6],
        ];
        assert!(flat(build(&|b| b.extrude_z(&wedge, 0.0, 1.0))));
        // A faceted arc in any loft is a curve: smooth round it, sharp at its corners.
        let mut arc: Vec<[f32; 2]> = (0..=8)
            .map(|i| {
                let a = (-80.0 + 20.0 * i as f32).to_radians();
                [a.cos(), a.sin()]
            })
            .collect();
        arc.extend([[-0.8, 0.9], [-0.8, -0.9]]);
        let helmet = build(&|b| b.extrude_z(&arc, 0.0, 1.0));
        let lean = most_lean(&helmet);
        assert!(lean > 5.0 && lean < 20.0, "arc leans {lean}");
    }

    #[test]
    fn a_bevelled_box_has_flat_faces_and_rounded_edges() {
        let size = Vec3::new(4.0, 3.0, 2.0);
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.with_bevel(0.2, |b| b.cuboid(Vec3::ZERO, size));
        let mesh = b.finish();
        assert!(mesh.indices.len() / 3 > 12, "the edges were cut");
        // Nothing sticks out past the box, and its corners are gone.
        let reach = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from(v.pos).abs())
            .fold(Vec3::ZERO, Vec3::max);
        assert!(
            (reach - size * 0.5).abs().max_element() < 1e-4,
            "reach {reach}"
        );
        assert!(
            mesh.vertices
                .iter()
                .all(
                    |v| (Vec3::from(v.pos).abs() - size * 0.5).max_element() < -1e-3
                        || (Vec3::from(v.pos).abs() - size * 0.5).min_element() < -0.05
                ),
            "a sharp corner survived"
        );
        let mut facing = 0;
        for t in mesh.indices.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.vertices[i as usize].pos);
            let n = triangle_normal(p(t[0]), p(t[1]), p(t[2])).normalize();
            let shading: Vec<Vec3> = t
                .iter()
                .map(|&i| Vec3::from(mesh.vertices[i as usize].normal))
                .collect();
            if n.abs().max_element() > 0.999 {
                // A broad face: its own normal, give or take the thin bevels beside it.
                assert!(shading.iter().all(|s| s.dot(n) > 0.99), "face {n} leans");
                facing += 1;
            } else {
                // A bevel: its corners turn from one face's normal toward the other's.
                let spread = shading
                    .iter()
                    .map(|s| shading.iter().map(|o| s.dot(*o)).fold(1.0, f32::min))
                    .fold(1.0, f32::min);
                assert!(spread < 0.9, "bevel at {} shades flat", p(t[0]));
            }
        }
        assert!(facing >= 12);
        // Only at full detail.
        let mut b = MeshBuilder::new(1, Affine3A::IDENTITY);
        b.with_bevel(0.2, |b| b.cuboid(Vec3::ZERO, size));
        assert_eq!(b.finish().indices.len(), 36);
    }

    #[test]
    fn a_curved_loft_gets_one_frame_round_it() {
        let ring: Vec<[f32; 2]> = ngon(12, 1.0);
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.extrude_z(&ring, 0.0, 2.0);
        let mesh = b.finish();
        // Tube frames are marked by a negative half width.
        assert!(
            mesh.vertices.iter().any(|v| v.face[2] < 0.0),
            "sides framed as one tube"
        );
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
            assert!(
                (s.abs() - hw).abs() < 1e-4 && (t.abs() - hh).abs() < 1e-4,
                "{v:?}"
            );
            let want = if v.normal[2].abs() > 0.5 {
                [4.0, 2.0]
            } else if v.normal[0].abs() > 0.5 {
                [2.0, 1.0]
            } else {
                [4.0, 1.0]
            };
            assert!(
                (hw - want[0]).abs() < 1e-4 && (hh - want[1]).abs() < 1e-4,
                "{v:?}"
            );
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
            &[
                [-10.0, 0.0],
                [10.0, 0.0],
                [10.0, 4.0],
                [2.0, 9.0],
                [-6.0, 9.0],
                [-10.0, 5.0],
            ],
            -3.0,
            3.0,
        );
        let mesh = b.finish();
        let gable: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.normal[1].abs() > 0.9)
            .collect();
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
        b.beam(
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 10.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 1.0),
        );
        let mesh = b.finish();
        let side: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.normal[1].abs() > 0.9)
            .collect();
        assert!(!side.is_empty());
        for v in side {
            let (long, short) = (v.face[2].max(v.face[3]), v.face[2].min(v.face[3]));
            assert!(
                (long - 50f32.sqrt()).abs() < 1e-3 && (short - 0.5).abs() < 1e-3,
                "{v:?}"
            );
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
            assert!(
                (v.face[2] + PI * 2.0).abs() < 1e-3,
                "a negative half width marks the wrap"
            );
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
        assert!(mesh.vertices[..24]
            .iter()
            .all(|v| v.surface & 0xFF == pattern::DECK));
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
