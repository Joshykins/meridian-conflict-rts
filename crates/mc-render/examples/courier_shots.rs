//! Courier visual acceptance: native GPU closeups with the ARC Commander for scale.
//! cargo run --release -p mc-render --example courier_shots -- maps/dev16.mcmap artifacts/courier
use std::{path::Path, sync::Arc};
use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_core::{Angle,FxVec2};
use mc_sim::{World,MatchConfig,PlayerSetup,Command,PlayerCommand,Handle};
use mc_sim::tables::Controller;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, UnitInstance};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    let spot = Vec2::from(map.start_positions()[0].to_f32());
    let overlay = Overlay::default();
    let mut renderer = Renderer::new(Target::Headless { width: 1280, height: 800 }, SceneDesc {
        map: map.clone(), blueprints: blueprints.clone(), pool: Arc::new(Pool::new(4)),
        team_colors: [[0.1, 0.45, 0.95]; 8],
    }).unwrap();
    let base = spot.extend(renderer.ground_height(spot));
    let make = |key: &str, pos: Vec3, unit_id: u32| {
        let id = blueprints.id_of(key).unwrap();
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = pos.to_array(); unit.prev_pos = unit.pos;
        unit.blueprint = id.index() as u32;
        unit.health = 1.0; unit.build = 1.0;
        unit.radius = blueprints.unit(id).radius.to_f32(); unit.unit_id = unit_id;
        unit
    };
    let mut frame = RenderFrame::default();
    frame.props_dead = vec![u32::MAX; map.props().len().div_ceil(32)];
    frame.units.push(make("aster_t1_lift_ship", base, 1));
    frame.units.push(make("aster_commander", base + Vec3::new(-82.0,-6.0,0.0), 2));
    let mut camera = Camera::new(size, Vec2::new(1280.0, 800.0));
    camera.focus = base + Vec3::new(-8.0, 0.0, 16.0);
    camera.distance = 235.0; camera.tilt = 0.48; camera.yaw = -0.8;
    for (shot, tilt, yaw, under) in [
        ("courier-scale",0.48,-0.8,false),
        ("courier-clearance",0.20,2.8,true),
        ("courier-rear",0.5,2.5,false),
        ("courier-doors",1.3,1.75,true),
    ] {
        camera.tilt = tilt; camera.yaw = yaw;
        // The stern close-ups frame the door portal from behind and low.
        if shot.starts_with("courier-doors") {
            camera.focus = base + Vec3::new(-44.0, 0.0, 12.0);
            camera.distance = 110.0;
        } else {
            camera.focus = base + Vec3::new(-8.0, 0.0, 16.0);
            camera.distance = 235.0;
        }
        frame.units[0].deploy = if under {1.0} else {0.0};
        frame.units[0].prev_deploy = frame.units[0].deploy;
        let pos = if under { base } else { base + Vec3::new(-82.0,-6.0,0.0) };
        frame.units[1].pos = pos.to_array(); frame.units[1].prev_pos = pos.to_array();
        for i in 0..8 {
            renderer.render(&FrameInput { camera: &camera, time: 10.0+i as f32*0.1, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        }
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = b"P6\n1280 800\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::write(out.join(format!("{shot}.ppm")),ppm).unwrap();
    }
    // Render actual sim state through boarding and unloading, including the exact
    // frame before stow and the first visible release, to catch popping/clipping.
    let config=MatchConfig {seed:3,players:vec![PlayerSetup {
        name:"Preview".into(),faction:"Aster".into(),ai:Default::default(),team:0,
        controller:Controller::Human,start:0}],cheats:true,fog:false,spawn_commanders:false};
    let mut world=World::new(&map,blueprints.clone(),Arc::new(Pool::new(1)),&config).unwrap();
    let pos=map.start_positions()[0];
    let ship=world.spawn_unit(blueprints.id_of("aster_t1_lift_ship").unwrap(),0,pos,Angle::ZERO,true).unwrap();
    let ground=world.terrain.height_at(pos);
    world.state.units.z[ship]=ground; world.state.units.prev_z[ship]=ground;
    let passenger=world.spawn_unit(blueprints.id_of("aster_commander").unwrap(),0,pos-FxVec2::from_ints(100,0),Angle::ZERO,true).unwrap();
    let sid=world.state.units.id(ship); let cid=world.state.units.id(passenger);
    let cmd=|command|PlayerCommand {player:0,command};
    world.tick(&[cmd(Command::Board{units:vec![cid],carrier:sid,queue:false})]).unwrap();
    camera.focus=base+Vec3::new(-22.0,0.0,15.0);camera.distance=205.0;camera.yaw=2.75;camera.tilt=0.28;
    let mut saved=0;
    for tick in 0..350 {
        world.tick(&[]).unwrap();
        if tick%8==0 {
            world.write_render_frame(None,&mut frame);
            renderer.render(&FrameInput{camera:&camera,time:tick as f32*0.1,alpha:1.0,
                sim:Some(&frame),ghosts:&[],marks:&[],ranges:&[],ranges_drawn:0,
                overlay:&overlay,build_grid:false}).unwrap();
            let pixels=renderer.read_pixels().unwrap();
            let mut ppm=b"P6\n1280 800\n255\n".to_vec();
            for pixel in pixels.chunks_exact(4){ppm.extend_from_slice(&pixel[..3]);}
            std::fs::write(out.join(format!("loading-{saved:03}.ppm")),ppm).unwrap();
            saved+=1;
        }
        if world.state.units.hangar[passenger]==sid {break;}
    }
    assert_eq!(world.state.units.hangar[passenger],sid,"preview commander never boarded");
    world.tick(&[cmd(Command::Land{units:vec![sid],pos,unload:true,queue:false})]).unwrap();
    for tick in 0..100 {
        world.tick(&[]).unwrap();
        if tick%8==0 {
            world.write_render_frame(None,&mut frame);
            renderer.render(&FrameInput{camera:&camera,time:35.0+tick as f32*0.1,alpha:1.0,
                sim:Some(&frame),ghosts:&[],marks:&[],ranges:&[],ranges_drawn:0,
                overlay:&overlay,build_grid:false}).unwrap();
            let pixels=renderer.read_pixels().unwrap();
            let mut ppm=b"P6\n1280 800\n255\n".to_vec();
            for pixel in pixels.chunks_exact(4){ppm.extend_from_slice(&pixel[..3]);}
            std::fs::write(out.join(format!("unloading-{:03}.ppm",tick/8)),ppm).unwrap();
        }
    }
    assert_eq!(world.state.units.hangar[passenger],Handle::NONE);
    println!("Commander boarded into eight slots and walked out; {saved} loading frames captured.");

}
