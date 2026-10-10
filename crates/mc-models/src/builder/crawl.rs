//! A many-legged walker's rig (`Crawl`): its legs, a tail, pincers and its brace.

use glam::Vec3;

use super::{rig, MeshBuilder};

impl MeshBuilder {
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
        assert!((1..=crate::MAX_CRAWL_PAIRS).contains(&pairs.len()));
        let (h, k, a, _) = pairs[0];
        self.set_legs(h, k, a, stride, stance, lift);
        let at = |p: Vec3| self.transform.transform_point3(p).to_array();
        let mut crawl = crate::Crawl {
            pairs: pairs.len(),
            joints: [[[0.0; 3]; 3]; crate::MAX_CRAWL_PAIRS],
            phase: [0.0; crate::MAX_CRAWL_PAIRS],
            stance: [stance; crate::MAX_CRAWL_PAIRS],
            tail: [0.0; 2],
            tail_joints: [[0.0; 2]; crate::MAX_TAIL_JOINTS],
            tail_count: 0,
            claw: None,
            throws: None,
            lone: None,
            brace: None,
        };
        for (i, &(hip, knee, ankle, phase)) in pairs.iter().enumerate() {
            crawl.joints[i] = [at(hip), at(knee), at(ankle)];
            crawl.phase[i] = phase.rem_euclid(1.0);
        }
        self.legs.as_mut().unwrap().crawl = Some(crawl);
    }

    /// Makes leg pair `pair` (`set_crawl_legs`) one leg on the centreline: its joints lie
    /// in y = 0, and both halves of it walk in step at the pair's own phase rather than half
    /// a cycle apart (a tripod's third leg). Its vertices still split by the side they lie
    /// on, so it must be symmetrical about y = 0.
    pub fn set_lone_leg(&mut self, pair: usize) {
        let crawl = self
            .legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first");
        assert!(pair < crawl.pairs);
        assert!(
            crawl.joints[pair].iter().all(|j| j[1].abs() < 1e-4),
            "a lone leg stands on the centreline"
        );
        crawl.lone = Some(pair);
    }

    /// Has leg pair `pair` (`set_crawl_legs`) stand `stance` of the cycle rather than the
    /// walker's own: a shorter stance is a shorter step at the same pace, and a longer
    /// swing to make it in.
    pub fn set_pair_stance(&mut self, pair: usize, stance: f32) {
        let crawl = self
            .legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first");
        assert!(pair < crawl.pairs && stance > 0.0 && stance < 1.0);
        crawl.stance[pair] = stance;
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
        assert!((2..=crate::MAX_TAIL_JOINTS).contains(&joints.len()));
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
        self.with_part(crate::part::TURRET, |b| b.with_limb(rig::TAIL, f));
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

    /// Has the walker (`set_crawl_legs`, authored braced) brace to fire with its deploy
    /// (`gpu_consts::crawl_brace`): packed, its body rides `sink` metres higher and its feet
    /// stand `widen` of the way in from where they are authored toward their hips; planting,
    /// each leg steps out in turn and the body settles between them.
    pub fn set_crawl_brace(&mut self, sink: f32, widen: f32) {
        assert!(sink >= 0.0 && (0.0..0.6).contains(&widen));
        let sink = self.transform.transform_vector3(Vec3::Z * sink).z;
        self.legs
            .as_mut()
            .and_then(|l| l.crawl.as_mut())
            .expect("set_crawl_legs first")
            .brace = Some([sink, widen]);
    }
}
