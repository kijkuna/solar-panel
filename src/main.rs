use bevy::{
    asset::RenderAssetUsages,
    image::{ImageLoaderSettings, ImageSampler},
    input::{
        mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
        touch::Touches,
    },
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

const EARTH_RADIUS: f32 = 1.0;
const MIN_CAMERA_DISTANCE: f32 = 1.35;
const MAX_CAMERA_DISTANCE: f32 = 45.0;

const MOUSE_SENSITIVITY: f32 = 0.0045;
const TOUCH_SENSITIVITY: f32 = 0.0040;
const ZOOM_SENSITIVITY: f32 = 0.13;
const MIN_PITCH: f32 = -1.0;
const MAX_PITCH: f32 = 1.0;

const SKY_RADIUS: f32 = 120.0;
const SKY_HALF_HEIGHT: f32 = 100_000.0;
const SKY_HORIZONTAL_SEGMENTS: u32 = 256;
const SKY_VERTICAL_SEGMENTS: u32 = 128;

#[derive(Component)]
struct OrbitCameraTag;

#[derive(Component)]
struct SkyPanoramaTag;

#[derive(Resource)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            // Camera starts on +Z: latitude 0°, longitude 0°.
            yaw: 0.0,
            pitch: 0.0,
            distance: 4.0,
        }
    }
}

#[derive(Resource, Default)]
struct TouchGestureState {
    last_tap_at: Option<f64>,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Earth Orbit — Bevy".to_string(),
                resolution: (1280, 800).into(),
                canvas: Some("#bevy-canvas".to_string()),
                fit_canvas_to_parent: true,
                // CSS touch-action:none in index.html prevents browser page gestures.
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .init_resource::<OrbitCamera>()
        .init_resource::<TouchGestureState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (orbit_camera, keep_panorama_centered).chain(),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Equirectangular Earth mesh with an explicit coordinate convention:
    // +Y = north pole, +Z = 0° latitude / 0° longitude, +X = 90° east.
    // This avoids relying on the default orientation of a built-in UV sphere.
    let earth_texture: Handle<Image> = asset_server.load("earth.png");
    let earth_mesh = meshes.add(make_earth_mesh(384, 192));

    let earth_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(earth_texture),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    });

    commands.spawn((
        Name::new("Earth"),
        Mesh3d(earth_mesh),
        MeshMaterial3d(earth_material),
        Transform::IDENTITY,
    ));

    // The source atlas is a 4x3 cross. Only its complete middle row is used:
    // the two vertical tiles above/below it are intentionally excluded.
    // The installer places a seam-matched, sharpened panorama in assets/.
    let sky_texture: Handle<Image> = asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::linear();
        })
        .load("space_panorama.png");

    let sky_mesh = meshes.add(make_sky_sphere_mesh());
    let sky_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(sky_texture),
        unlit: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Name::new("Camera-centered space panorama"),
        SkyPanoramaTag,
        Mesh3d(sky_mesh),
        MeshMaterial3d(sky_material),
        Transform::IDENTITY,
    ));

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-4.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let orbit = OrbitCamera::default();
    let camera_position = orbit_position(&orbit);
    commands.spawn((
        Name::new("Orbit camera"),
        OrbitCameraTag,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            far: 2_500.0,
            ..default()
        }),
        Transform::from_translation(camera_position).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn orbit_camera(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    keyboard: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    time: Res<Time>,
    mut touch_state: ResMut<TouchGestureState>,
    mut orbit: ResMut<OrbitCamera>,
    mut camera: Single<&mut Transform, (With<OrbitCameraTag>, Without<SkyPanoramaTag>)>,
) {
    // Desktop: hold the left mouse button and drag.
    if mouse_buttons.pressed(MouseButton::Left) {
        orbit.yaw -= mouse_motion.delta.x * MOUSE_SENSITIVITY;

        // Inverted vertical axis: dragging down tilts the virtual camera upward.
        orbit.pitch += mouse_motion.delta.y * MOUSE_SENSITIVITY;
    }

    // Desktop / trackpad zoom. Positive wheel delta zooms in.
    let scroll_y = match mouse_scroll.unit {
        MouseScrollUnit::Line => mouse_scroll.delta.y,
        MouseScrollUnit::Pixel => mouse_scroll.delta.y / 100.0,
    };
    if scroll_y.abs() > 0.0 {
        orbit.distance *= (-scroll_y * ZOOM_SENSITIVITY).exp();
    }

    // Mobile:
    // - one finger drag: orbit
    // - two finger pinch: zoom
    // - double tap: reset camera
    let mut active_touches: Vec<_> = touches.iter().collect();
    active_touches.sort_by_key(|touch| touch.id());

    match active_touches.as_slice() {
        [finger] => {
            let delta = finger.delta();
            orbit.yaw -= delta.x * TOUCH_SENSITIVITY;
            orbit.pitch -= delta.y * TOUCH_SENSITIVITY;
        }
        [first, second, ..] => {
            let previous_distance = first
                .previous_position()
                .distance(second.previous_position());
            let current_distance = first.position().distance(second.position());

            if previous_distance > 1.0 && current_distance > 1.0 {
                orbit.distance *= previous_distance / current_distance;
            }
        }
        _ => {}
    }

    // A deliberate double tap is a convenient mobile equivalent of Home.
    for touch in touches.iter_just_released() {
        if touch.distance().length() <= 18.0 {
            let now = time.elapsed_secs_f64();
            match touch_state.last_tap_at {
                Some(previous_tap) if now - previous_tap <= 0.38 => {
                    *orbit = OrbitCamera::default();
                    touch_state.last_tap_at = None;
                }
                _ => touch_state.last_tap_at = Some(now),
            }
        }
    }

    if keyboard.just_pressed(KeyCode::Home) {
        *orbit = OrbitCamera::default();
    }

    orbit.pitch = orbit.pitch.clamp(MIN_PITCH, MAX_PITCH);
    orbit.distance = orbit
        .distance
        .clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE);

    let position = orbit_position(&orbit);
    **camera = Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y);
}

fn orbit_position(orbit: &OrbitCamera) -> Vec3 {
    let cos_pitch = orbit.pitch.cos();
    Vec3::new(
        orbit.distance * cos_pitch * orbit.yaw.sin(),
        orbit.distance * orbit.pitch.sin(),
        orbit.distance * cos_pitch * orbit.yaw.cos(),
    )
}

/// Follow camera translation only. The panorama keeps its world orientation,
/// so panning the camera changes the viewed direction but camera translation
/// cannot create parallax or make the background appear to move closer/farther.
fn keep_panorama_centered(
    camera: Single<&Transform, (With<OrbitCameraTag>, Without<SkyPanoramaTag>)>,
    mut panorama: Single<&mut Transform, (With<SkyPanoramaTag>, Without<OrbitCameraTag>)>,
) {
    panorama.translation = camera.translation;
}

/// A single continuous, uncapped cylinder (no top or bottom faces).
/// The full 4:1 panorama is wrapped around its circumference.


fn make_sky_sphere_mesh() -> Mesh {
    let sectors = 256;
    let stacks = 128;

    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();

    let stride = sectors + 1;

    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let latitude = std::f32::consts::FRAC_PI_2
            - v * std::f32::consts::PI;

        let cos_lat = latitude.cos();
        let sin_lat = latitude.sin();

        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let longitude = (u - 0.5) * std::f32::consts::TAU;

            let normal = Vec3::new(
                cos_lat * longitude.sin(),
                sin_lat,
                cos_lat * longitude.cos(),
            ).normalize();

            positions.push((normal * SKY_RADIUS).to_array());
            normals.push((-normal).to_array());
            uvs.push([u, v]);
        }
    }

    for stack in 0..stacks {
        for sector in 0..sectors {
            let a = stack * stride + sector;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;

            indices.extend_from_slice(&[
                a, b, c,
                b, d, c,
            ]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Custom equirectangular sphere:
/// - U=0.5 is the zero meridian, facing +Z.
/// - V=0 is the north pole, V=0.5 the equator, V=1 the south pole.
fn make_earth_mesh(sectors: u32, stacks: u32) -> Mesh {
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();

    let stride = sectors + 1;
    let vertex_count = (stacks + 1) * stride;
    positions.reserve(vertex_count as usize);
    normals.reserve(vertex_count as usize);
    uvs.reserve(vertex_count as usize);

    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let latitude = std::f32::consts::FRAC_PI_2 - v * std::f32::consts::PI;
        let cos_lat = latitude.cos();
        let sin_lat = latitude.sin();

        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let longitude = (u - 0.5) * std::f32::consts::TAU;

            let normal = Vec3::new(
                cos_lat * longitude.sin(),
                sin_lat,
                cos_lat * longitude.cos(),
            )
            .normalize();

            positions.push((normal * EARTH_RADIUS).to_array());
            normals.push(normal.to_array());
            uvs.push([u, v]);
        }
    }

    for stack in 0..stacks {
        for sector in 0..sectors {
            let a = stack * stride + sector;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;

            // Counter-clockwise from outside: normals point away from center.
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
