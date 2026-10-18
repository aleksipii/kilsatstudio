//! Adapter-free commands for the explicitly incomplete experimental GPU renderer.
//! No font rasterizer is provided: actor labels require explicit acknowledgement.
use motion_lipsync::Viseme;
use motion_scene::{EvaluatedScene, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};

pub const MAX_COMMANDS: usize = 4096;
/// Bound the analytic fragment shader's command visits, not merely allocation.
pub const MAX_FRAGMENT_WORK: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Labels {
    #[default]
    Require,
    /// Explicit geometry-only experiment; this is not SVG/native parity.
    OmitAcknowledged,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Scene(#[from] crate::Error),
    #[error(transparent)]
    Gpu(#[from] crate::gpu::Error),
    #[error("unsupported experimental GPU feature: {0}")]
    UnsupportedFeature(&'static str),
    #[error("experimental GPU geometry or fragment-work budget exceeded")]
    Budget,
}

/// Five vec4 values, serialized explicitly: no unsafe casts or host-layout assumptions.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Command(pub [[f32; 4]; 5]);

#[derive(Debug)]
pub struct SceneCommands {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) canvas: [f32; 4],
    pub(crate) background: [f32; 4],
    pub(crate) opacity: f32,
    pub(crate) commands: Vec<Command>,
}
impl SceneCommands {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
    pub(crate) fn bytes(&self) -> Vec<u8> {
        self.commands.iter().flat_map(|c| c.0.iter().flatten().flat_map(|v| v.to_le_bytes())).collect()
    }
}

/// Only an evaluated actor scene is accepted. Artwork/sequence callers must use
/// the explicit unsupported entry point rather than drop their extra content.
pub fn build(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    labels: Labels,
) -> Result<SceneCommands, Error> {
    crate::validate_scene(scene, width, height)?;
    crate::gpu::row_layout(width, height)?;
    if mouths.len() > crate::MAX_ACTORS { return Err(Error::Budget); }
    if !scene.actors.is_empty() && labels == Labels::Require {
        return Err(Error::UnsupportedFeature("actor font labels; select OmitAcknowledged for geometry-only experiments"));
    }
    // Reserve a conservative upper bound before expanding any actor geometry.
    let upper = scene.actors.len().checked_mul(16).ok_or(Error::Budget)?;
    if upper > MAX_COMMANDS || u64::from(width) * u64::from(height) * (upper as u64 + 1) > MAX_FRAGMENT_WORK {
        return Err(Error::Budget);
    }
    let scale = (width as f64 / scene.width as f64).min(height as f64 / scene.height as f64);
    let ox = (width as f64 - scene.width as f64 * scale) * 0.5;
    let oy = (height as f64 - scene.height as f64 * scale) * 0.5;
    let byte = |i| u8::from_str_radix(&scene.background_color[i..i+2], 16).unwrap() as f32 / 255.0;
    let mut result = SceneCommands {
        width, height,
        canvas: [ox as f32, oy as f32, (scene.width as f64 * scale) as f32, (scene.height as f64 * scale) as f32],
        background: [byte(1), byte(3), byte(5), 1.0],
        opacity: scene.opacity as f32,
        commands: Vec::with_capacity(upper),
    };
    let (cx, cy) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    for actor in &scene.actors {
        let factor = scale * scene.camera.zoom;
        let (sin, cos) = actor.rotation.sin_cos();
        let a = cos * actor.scale.x * factor;
        let b = sin * actor.scale.x * factor;
        let c = -sin * actor.scale.y * factor;
        let d = cos * actor.scale.y * factor;
        let tx = ox + scale * (cx + scene.camera.zoom * actor.x);
        let ty = oy + scale * (cy + scene.camera.zoom * actor.y);
        let det = a*d-b*c;
        let inv = [(d/det) as f32, (-b/det) as f32, (-c/det) as f32, (a/det) as f32];
        let origin = [((-d*tx+c*ty)/det) as f32, ((b*tx-a*ty)/det) as f32,
            (0.75 / (actor.scale.x.min(actor.scale.y) * factor)) as f32, 0.0];
        let mut push = |kind: f32, geom: [f32;4], color: [f32;4], radius: f32| {
            result.commands.push(Command([inv, origin, geom, color, [kind, radius, 0.0, 0.0]]));
        };
        let bone = [126.0/255.0,185.0/255.0,226.0/255.0,1.0];
        let skin = [245.0/255.0,200.0/255.0,162.0/255.0,1.0];
        let mouth = [125.0/255.0,45.0/255.0,57.0/255.0,1.0];
        let black = [0.0,0.0,0.0,1.0];
        let p = &actor.skeleton;
        for (a,b) in [(HIP,CHEST),(CHEST,HEAD),(CHEST,LEFT_HAND),(CHEST,RIGHT_HAND),(HIP,LEFT_FOOT),(HIP,RIGHT_FOOT)] {
            push(2.0,[p[a].x as f32,p[a].y as f32,p[b].x as f32,p[b].y as f32],bone,2.5);
        }
        let hx=p[HEAD].x as f32;
        let hy=p[HEAD].y as f32;
        push(0.0,[hx,hy,25.0,0.0],skin,0.0);
        push(0.0,[hx-8.0,hy-4.0,2.5,0.0],black,0.0);
        push(0.0,[hx+8.0,hy-4.0,2.5,0.0],black,0.0);
        let viseme=mouths.iter().find(|(id,_)| *id==actor.id).map(|(_,v)| *v).unwrap_or(Viseme::Rest);
        let (w,h)=match viseme {
            Viseme::A=>(14.0,14.0), Viseme::O|Viseme::U=>(8.0,11.0),
            Viseme::E|Viseme::I=>(18.0,6.0), Viseme::Rest|Viseme::Closed=>(13.0,2.0), _=>(12.0,5.0),
        };
        push(1.0,[hx,hy+10.0,w/2.0,h/2.0],mouth,2.0_f32.min(h/2.0));
        if actor.emotion=="happy" && matches!(viseme,Viseme::Rest|Viseme::Closed) {
            push(3.0,[hx-8.0,hy+9.0,hx,hy+14.0],mouth,1.0);
            push(3.0,[hx,hy+14.0,hx+8.0,hy+9.0],mouth,1.0);
        }
        if actor.emotion=="surprised" {
            push(3.0,[hx-12.0,hy-13.0,hx-4.0,hy-15.0],black,0.5);
            push(3.0,[hx+4.0,hy-13.0,hx+12.0,hy-15.0],black,0.5);
        }
    }
    debug_assert!(result.commands.len() <= upper);
    Ok(result)
}

pub fn reject_artwork_or_sequence() -> Error {
    Error::UnsupportedFeature("artwork, image textures, masks, blend modes and sequence transitions")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> EvaluatedScene { crate::tests::scene() }
    #[test]
    fn labels_cannot_silently_disappear() {
        assert!(matches!(build(&scene(),640,360,&[],Labels::Require),Err(Error::UnsupportedFeature(_))));
    }
    #[test]
    fn camera_root_and_letterbox_are_shared_scene_transforms() {
        let mut s=scene(); s.camera.x=30.0; s.camera.y=-10.0; s.camera.zoom=1.5;
        s.actors[0].rotation=0.37; s.actors[0].scale=motion_scene::Vec2::new(1.3,0.7);
        let commands=build(&s,640,480,&[],Labels::OmitAcknowledged).unwrap();
        assert_eq!(commands.canvas,[0.0,60.0,640.0,360.0]);
        let world=s.actors[0].local_to_world(s.actors[0].skeleton[HEAD]);
        let (x,y)=s.camera.project(world.x,world.y,s.width,s.height);
        let c=&commands.commands[6].0;
        let local_x=c[0][0]*x as f32+c[0][2]*(y+60.0) as f32+c[1][0];
        let local_y=c[0][1]*x as f32+c[0][3]*(y+60.0) as f32+c[1][1];
        assert!((local_x-c[2][0]).abs()<0.001); assert!((local_y-c[2][1]).abs()<0.001);
    }
    #[test]
    fn scalar_validation_and_fragment_work_precede_adapter_creation() {
        let mut s=scene(); s.opacity=f64::NAN;
        assert!(matches!(build(&s,640,360,&[],Labels::OmitAcknowledged),Err(Error::Scene(_))));
        let s=scene(); assert!(matches!(build(&s,2048,2048,&[],Labels::OmitAcknowledged),Err(Error::Budget)));
        let mouths=vec![("EMMA",Viseme::Rest);crate::MAX_ACTORS+1];
        assert!(matches!(build(&s,64,64,&mouths,Labels::OmitAcknowledged),Err(Error::Budget)));
    }
    #[test]
    fn empty_scene_background_opacity_and_serialization() {
        let mut s=scene(); s.actors.clear(); s.opacity=0.25;
        let c=build(&s,65,64,&[],Labels::Require).unwrap();
        assert_eq!(c.command_count(),0); assert_eq!(c.dimensions(),(65,64));
        assert_eq!(c.opacity,0.25); assert!(c.bytes().is_empty());
        let c=build(&scene(),640,360,&[],Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.bytes().len(),c.command_count()*80);
    }
    #[test]
    fn authored_skeleton_offsets_reach_gpu_commands_without_reevaluation() {
        let mut s=scene();
        s.actors[0].skeleton[RIGHT_HAND]=motion_scene::Vec2::new(99.0,-77.0);
        let c=build(&s,640,360,&[],Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.commands[3].0[2][2..],[99.0,-77.0]);
        s.opacity=0.375;
        let c=build(&s,640,360,&[],Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.opacity,0.375);
    }
    #[test]
    fn mouth_dimensions_and_actor_order_are_deterministic() {
        let mut s=scene(); s.actors[0].emotion="neutral".into();
        for (v,half) in [(Viseme::A,[7.0,7.0]),(Viseme::O,[4.0,5.5]),(Viseme::U,[4.0,5.5]),(Viseme::E,[9.0,3.0]),(Viseme::I,[9.0,3.0]),(Viseme::Rest,[6.5,1.0]),(Viseme::Closed,[6.5,1.0]),(Viseme::TeethLip,[6.0,2.5]),(Viseme::TongueTeeth,[6.0,2.5]),(Viseme::TongueUp,[6.0,2.5]),(Viseme::Sibilant,[6.0,2.5]),(Viseme::Sh,[6.0,2.5]),(Viseme::R,[6.0,2.5])] {
            let c=build(&s,640,360,&[("EMMA",v)],Labels::OmitAcknowledged).unwrap();
            assert_eq!(c.commands[9].0[2][2..],half);
        }
        let mut second=s.actors[0].clone(); second.id="ALEX".into(); second.x+=100.0; s.actors.push(second);
        let c=build(&s,640,360,&[],Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.command_count(),20); assert_ne!(c.commands[0].0[1],c.commands[10].0[1]);
    }
}
