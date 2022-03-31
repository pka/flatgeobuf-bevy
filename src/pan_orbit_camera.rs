use crate::instant::Instant;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

/// Tags an entity as capable of panning and orbiting.
pub struct PanOrbitCamera {
    /// The "focus point" to orbit around. It is automatically updated when panning the camera
    pub focus: Vec3,
}

impl Default for PanOrbitCamera {
    fn default() -> Self {
        PanOrbitCamera {
            focus: Vec3::zero(),
        }
    }
}

/// Hold readers for events
#[derive(Default)]
pub struct InputState {
    pub reader_motion: EventReader<MouseMotion>,
    // Web: We get CursorMoved instead of MouseMotion events
    pub reader_cursor: EventReader<CursorMoved>,
    // Web: First position after pressing mouse button
    pub cursor_startpos: Option<Vec2>,
    // Timestamp when motions begins
    pub last_motion: Option<Instant>,
    pub reader_scroll: EventReader<MouseWheel>,
    // Timestamp when scroll begins
    pub last_zoom: Option<Instant>,
}

const PAN_FACTOR: f32 = 100.0;
const PAN_FACTOR_WEB: f32 = 2.0;

/// Pan the camera with LHold or scrollwheel, orbit with rclick.
fn pan_orbit_camera(
    time: Res<Time>,
    windows: Res<Windows>,
    mut state: ResMut<InputState>,
    ev_motion: Res<Events<MouseMotion>>,
    ev_cursor: Res<Events<CursorMoved>>,
    mousebtn: Res<Input<MouseButton>>,
    ev_scroll: Res<Events<MouseWheel>>,
    mut query: Query<(&mut PanOrbitCamera, &mut Transform)>,
) {
    let mut translation = Vec2::zero();
    let mut rotation_move = Vec2::default();
    let mut scroll = 0.0;
    let dt = time.delta_seconds;

    if mousebtn.pressed(MouseButton::Right) {
        for ev in state.reader_motion.iter(&ev_motion) {
            rotation_move += ev.delta;
        }
        // Web: absolute position instead of delta
        for ev in state.reader_cursor.iter(&ev_cursor) {
            if let Some(startpos) = state.cursor_startpos {
                rotation_move.x = (ev.position.x - startpos.x) * PAN_FACTOR_WEB;
                rotation_move.y = (startpos.y - ev.position.y) * PAN_FACTOR_WEB;
            } else {
                state.cursor_startpos = Some(ev.position);
            }
        }
    } else if mousebtn.pressed(MouseButton::Left) {
        // Pan only if we're not rotating at the moment
        for ev in state.reader_motion.iter(&ev_motion) {
            translation += ev.delta * PAN_FACTOR;
            state.last_motion = Some(Instant::now());
        }
        // Web: absolute position instead of delta
        for ev in state.reader_cursor.iter(&ev_cursor) {
            if let Some(startpos) = state.cursor_startpos {
                translation.x = (ev.position.x - startpos.x) * PAN_FACTOR_WEB;
                translation.y = (startpos.y - ev.position.y) * PAN_FACTOR_WEB;
            } else {
                state.cursor_startpos = Some(ev.position);
            }
        }
    } else {
        state.cursor_startpos = None;
    }

    for ev in state.reader_scroll.iter(&ev_scroll) {
        scroll += ev.y;
    }

    // Either pan+scroll or arcball. We don't do both at once.
    for (mut camera, mut trans) in query.iter_mut() {
        if rotation_move.length_squared() > 0.0 {
            let window = windows.get_primary().unwrap();
            let window_w = window.width() as f32;
            let window_h = window.height() as f32;

            // Link virtual sphere rotation relative to window to make it feel nicer
            let delta_x = rotation_move.x / window_w * std::f32::consts::PI * 2.0;
            let delta_y = rotation_move.y / window_h * std::f32::consts::PI;

            let delta_yaw = Quat::from_rotation_y(delta_x);
            let delta_pitch = Quat::from_rotation_x(delta_y);

            trans.translation =
                delta_yaw * delta_pitch * (trans.translation - camera.focus) + camera.focus;

            let look = Mat4::face_toward(trans.translation, camera.focus, Vec3::new(0.0, 1.0, 0.0));
            trans.rotation = look.to_scale_rotation_translation().1;
        } else {
            // The plane is x/y while z is "up". Multiplying by dt allows for a constant pan rate
            let mut translation = Vec3::new(-translation.x * dt, translation.y * dt, 0.0);
            camera.focus += translation;
            translation.z = -scroll;
            trans.translation += translation;
            if scroll != 0.0 {
                state.last_zoom = Some(Instant::now());
            }
        }
    }
}

fn spawn_camera2d(commands: &mut Commands) {
    commands
        .spawn((PanOrbitCamera::default(),))
        .with_bundle(Camera2dBundle::default());
}

pub struct PanOrbitCameraPlugin;

impl Plugin for PanOrbitCameraPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_resource(InputState::default())
            .add_system(spawn_camera2d.system())
            .add_system(pan_orbit_camera.system());
    }
}
