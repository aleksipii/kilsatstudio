//! Deterministic keyframes and an original procedural seven-joint humanoid.
pub use motion_rig::{Transform2,Vec2};
use motion_rig::{Joint,Rig};
pub const HIP:usize=0; pub const CHEST:usize=1; pub const HEAD:usize=2; pub const LEFT_HAND:usize=3; pub const RIGHT_HAND:usize=4; pub const LEFT_FOOT:usize=5; pub const RIGHT_FOOT:usize=6;
#[derive(Debug,Clone,Copy,PartialEq)] pub enum Easing {Linear,SmoothStep,Hold}
#[derive(Debug,Clone,Copy)] pub struct Keyframe {pub time:f64,pub value:Transform2,pub easing:Easing}
#[derive(Debug,Clone)] pub struct Track {keys:Vec<Keyframe>}
#[derive(Debug,thiserror::Error,PartialEq,Eq)] pub enum Error {
 #[error("track needs 1..=4096 strictly increasing finite nonnegative keys with valid transforms")] InvalidTrack,
 #[error("animation time must be finite and nonnegative")] InvalidTime,
}
impl Track {
 pub fn new(keys:Vec<Keyframe>)->Result<Self,Error>{if keys.is_empty()||keys.len()>4096||keys.iter().any(|k|!k.time.is_finite()||k.time<0.0||!k.value.is_valid())||keys.windows(2).any(|k|k[0].time>=k[1].time){return Err(Error::InvalidTrack)}Ok(Self{keys})}
 pub fn sample(&self,time:f64)->Result<Transform2,Error>{if !time.is_finite()||time<0.0{return Err(Error::InvalidTime)}let i=self.keys.partition_point(|k|k.time<=time);if i==0{return Ok(self.keys[0].value)}if i==self.keys.len(){return Ok(self.keys[i-1].value)}let (a,b)=(self.keys[i-1],self.keys[i]);let t=(time-a.time)/(b.time-a.time);let t=match a.easing{Easing::Linear=>t,Easing::SmoothStep=>t*t*(3.0-2.0*t),Easing::Hold=>0.0};let lerp=|a:f64,b:f64|a+(b-a)*t;let rotation_delta=(b.value.rotation-a.value.rotation+std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)-std::f64::consts::PI;Ok(Transform2{translation:Vec2::new(lerp(a.value.translation.x,b.value.translation.x),lerp(a.value.translation.y,b.value.translation.y)),rotation:a.value.rotation+rotation_delta*t,scale:Vec2::new(lerp(a.value.scale.x,b.value.scale.x),lerp(a.value.scale.y,b.value.scale.y))})}
}
/// Local skeleton points. Walk phase is elapsed seconds since the move began;
/// the wave envelope is supplied by the shared scene clock.
pub fn humanoid(walk_seconds:Option<f64>,wave:f64)->[Vec2;7]{
 let positions=[(0.0,-10.0,None),(0.0,-40.0,Some(HIP)),(0.0,-40.0,Some(CHEST)),(-30.0,30.0,Some(CHEST)),(30.0,30.0,Some(CHEST)),(-20.0,35.0,Some(HIP)),(20.0,35.0,Some(HIP))];
 let joints=positions.iter().enumerate().map(|(i,&(x,y,parent))|Joint{name:format!("joint_{i}"),parent,rest:Transform2{translation:Vec2::new(x,y),..Default::default()}}).collect();
 let rig=Rig::new(joints).expect("built-in rig is valid");let mut pose=rig.rest_pose();
 let stride=walk_seconds.filter(|t|t.is_finite()&&*t>=0.0).map_or(0.0,|t|(t*std::f64::consts::TAU*1.5).sin());
 pose[HIP].translation.y-=stride.abs()*2.0;
 pose[LEFT_FOOT].translation.x+=stride*13.0;pose[RIGHT_FOOT].translation.x-=stride*13.0;
 pose[LEFT_HAND].translation.x-=stride*7.0;pose[RIGHT_HAND].translation.x+=stride*7.0;
 let wave=if wave.is_finite(){wave.clamp(0.0,1.0)}else{0.0};pose[RIGHT_HAND].translation.y-=wave*65.0;
 let world=rig.evaluate(&pose).expect("bounded built-in pose");std::array::from_fn(|i|world[i].point(Vec2::default()))
}
#[cfg(test)] mod tests {use super::*;
 fn key(t:f64,x:f64,r:f64,easing:Easing)->Keyframe{Keyframe{time:t,value:Transform2{translation:Vec2::new(x,0.0),rotation:r,..Default::default()},easing}}
 #[test] fn interpolation_boundaries_shortest_rotation_and_easing(){let track=Track::new(vec![key(0.0,0.0,170f64.to_radians(),Easing::Linear),key(2.0,10.0,(-170f64).to_radians(),Easing::Linear)]).unwrap();let mid=track.sample(1.0).unwrap();assert_eq!(mid.translation.x,5.0);assert!((mid.rotation-std::f64::consts::PI).abs()<1e-10);assert_eq!(track.sample(2.0).unwrap().translation.x,10.0);assert_eq!(track.sample(9.0).unwrap().translation.x,10.0);let smooth=Track::new(vec![key(0.0,0.0,0.0,Easing::SmoothStep),key(1.0,1.0,0.0,Easing::Linear)]).unwrap();assert_eq!(smooth.sample(0.25).unwrap().translation.x,0.15625);let hold=Track::new(vec![key(0.0,0.0,0.0,Easing::Hold),key(1.0,1.0,0.0,Easing::Linear)]).unwrap();assert_eq!(hold.sample(0.9).unwrap().translation.x,0.0);assert_eq!(hold.sample(1.0).unwrap().translation.x,1.0);}
 #[test] fn rejects_bad_times_and_keys(){assert!(Track::new(vec![]).is_err());assert!(Track::new(vec![key(0.0,0.0,0.0,Easing::Linear);2]).is_err());let t=Track::new(vec![key(0.0,0.0,0.0,Easing::Linear)]).unwrap();assert!(t.sample(f64::NAN).is_err());assert!(t.sample(-1.0).is_err());}
 #[test] fn humanoid_is_seekable_and_preserves_hierarchy(){let idle=humanoid(None,0.0);assert_eq!(idle[HEAD],Vec2::new(0.0,-90.0));let walk=humanoid(Some(1.0/6.0),0.0);assert_ne!(walk,idle);assert_eq!(walk,humanoid(Some(1.0/6.0),0.0));assert_eq!(humanoid(None,1.0)[RIGHT_HAND].y,-85.0);assert_eq!(humanoid(None,f64::NAN),idle);}
}
