use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

const EARTH_RADIUS: f32 = 1.0;
const SKYBOX_HALF_SIZE: f32 = 250.0;
const MIN_CAMERA_DISTANCE: f32 = 1.45;
const MAX_CAMERA_DISTANCE: f32 = 35.0;
const MOUSE_SENSITIVITY: f32 = 0.005;
const ZOOM_SENSITIVITY: f32 = 0.14;

#[derive(Resource)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.22,
            distance: 4.0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Earth Orbit — Bevy".to_string(),
                resolution: (1280, 800).into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<OrbitCamera>()
        .add_systems(Startup, setup)
        .add_systems(Update, orbit_camera)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Земля: ортогональная (equirectangular) текстура 2:1.
    let earth_texture: Handle<Image> = asset_server.load("earth.jpeg");
    let earth_mesh = meshes.add(Sphere::new(EARTH_RADIUS).mesh().uv(128, 64));

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
    ));

    // Скайбокс: шесть граней извлекаются из cross-развёртки 4x3.
    let space_texture: Handle<Image> = asset_server.load("space.jpeg");
    let skybox_mesh = meshes.add(make_cross_skybox_mesh());
    let skybox_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(space_texture),
        unlit: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Name::new("Space skybox"),
        Mesh3d(skybox_mesh),
        MeshMaterial3d(skybox_material),
    ));

    // Направленное освещение подчёркивает объём шара.
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Камера всегда смотрит в центр Земли.
    let orbit = OrbitCamera::default();
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    commands.spawn((
        Name::new("Orbit camera"),
        Camera3d::default(),
        Transform {
            rotation,
            translation: rotation * Vec3::new(0.0, 0.0, orbit.distance),
            ..default()
        },
    ));

    // Короткая подсказка поверх сцены.
    commands.spawn((
        Text::new(
            "ЛКМ + движение мыши — вращение камеры\n\
             Колесо мыши — приближение / отдаление\n\
             Home — сброс положения камеры",
        ),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(16),
            left: px(16),
            ..default()
        },
    ));
}

fn orbit_camera(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut orbit: ResMut<OrbitCamera>,
    mut camera: Single<&mut Transform, With<Camera>>,
) {
    // Вращаем камеру только при зажатой левой кнопке мыши.
    if mouse_buttons.pressed(MouseButton::Left) {
        orbit.yaw -= mouse_motion.delta.x * MOUSE_SENSITIVITY;
        orbit.pitch = (orbit.pitch + mouse_motion.delta.y * MOUSE_SENSITIVITY)
            .clamp(-1.52, 1.52);
    }

    // Учитываем различие между прокруткой колёсика и прокруткой тачпада.
    let scroll_y = match mouse_scroll.unit {
        MouseScrollUnit::Line => mouse_scroll.delta.y,
        MouseScrollUnit::Pixel => mouse_scroll.delta.y / 100.0,
    };

    orbit.distance = (orbit.distance * (-scroll_y * ZOOM_SENSITIVITY).exp())
        .clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE);

    if keyboard.just_pressed(KeyCode::Home) {
        *orbit = OrbitCamera::default();
    }

    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    camera.rotation = rotation;
    camera.translation = rotation * Vec3::new(0.0, 0.0, orbit.distance);
}

/// Создаёт куб из 6 отдельных четырёхугольников.
///
/// Раскладка исходного space.jpeg (вид со стороны картинки):
///
///       [ ][ ][+Y][ ]
///       [-X][+Z][+X][-Z]
///       [ ][ ][-Y][ ]
///
/// Размер изображения 1020x768, то есть сетка из тайлов 255x256 px.
fn make_cross_skybox_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(24);
    let mut indices: Vec<u32> = Vec::with_capacity(36);

    // Направления right/up задают ориентацию каждой грани при взгляде изнутри куба.
    // Координаты тайлов указаны в столбцах и строках с нуля, сверху вниз.
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::X, Vec3::Z, Vec3::Y, 2, 1);
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::NEG_X, Vec3::NEG_Z, Vec3::Y, 0, 1);
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::Y, Vec3::NEG_X, Vec3::NEG_Z, 2, 0);
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::NEG_Y, Vec3::NEG_X, Vec3::Z, 2, 2);
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::Z, Vec3::NEG_X, Vec3::Y, 1, 1);
    push_sky_face(&mut positions, &mut normals, &mut uvs, &mut indices,
        Vec3::NEG_Z, Vec3::X, Vec3::Y, 3, 1);

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn push_sky_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    normal: Vec3,
    right: Vec3,
    up: Vec3,
    tile_column: u32,
    tile_row: u32,
) {
    let center = normal * SKYBOX_HALF_SIZE;
    let half = SKYBOX_HALF_SIZE;

    // Порядок вершин: верхний левый, верхний правый,
    // нижний правый, нижний левый.
    let corners = [
        center - right * half + up * half,
        center + right * half + up * half,
        center + right * half - up * half,
        center - right * half - up * half,
    ];

    let base = positions.len() as u32;
    for corner in corners {
        positions.push(corner.to_array());
        normals.push(normal.to_array());
    }

    // Учитываем половину пикселя по краям тайла, чтобы линейная фильтрация
    // не захватывала соседние грани atlas-текстуры.
    const IMAGE_WIDTH: f32 = 1020.0;
    const IMAGE_HEIGHT: f32 = 768.0;
    const TILE_WIDTH: f32 = IMAGE_WIDTH / 4.0;
    const TILE_HEIGHT: f32 = IMAGE_HEIGHT / 3.0;
    let u0 = (tile_column as f32 * TILE_WIDTH + 0.5) / IMAGE_WIDTH;
    let u1 = ((tile_column as f32 + 1.0) * TILE_WIDTH - 0.5) / IMAGE_WIDTH;
    let v0 = (tile_row as f32 * TILE_HEIGHT + 0.5) / IMAGE_HEIGHT;
    let v1 = ((tile_row as f32 + 1.0) * TILE_HEIGHT - 0.5) / IMAGE_HEIGHT;

    uvs.extend_from_slice(&[
        [u0, v0],
        [u1, v0],
        [u1, v1],
        [u0, v1],
    ]);

    indices.extend_from_slice(&[
        base, base + 1, base + 2,
        base, base + 2, base + 3,
    ]);
}
