use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    utils::info,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_rapier3d::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                player_movement,
                mouse_look,
                update_camera_fov,
                shoot_bullet,
                update_bullets,
                bullet_collision,
                enemy_shoots,
                update_aiming_state,
                update_hit_marker,
            ),
        )
        .run();
}

#[derive(Component)]
struct HitMarker;

#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

#[derive(Resource)]
struct Sensitivity {
    horizontal: f32,
    vertical: f32,
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct FpsCamera;

#[derive(Resource)]
struct CameraState {
    pitch: f32,
}

#[derive(Component)]
struct Bullet {
    lifetime: f32,
}

#[derive(Component)]
struct GunBarrel;

#[derive(Resource)]
struct RecoilState {
    vertical: f32,
    horizontal: f32,
}

#[derive(Component)]
struct Crosshair;

#[derive(Resource)]
struct ShootTimer(Timer);

#[derive(Component)]
struct Enemy;

#[derive(Resource)]
struct EnemyShootTimer(Timer);

#[derive(Component, PartialEq, Debug, Eq, Clone, Copy)]
enum Shooter {
    Player,
    Enemy,
}

#[derive(Resource)]
struct AimState {
    aiming: bool,
    transition: f32, // between 0.0 (hipfire) and 1.0 (ADS)
}

#[derive(Resource)]
struct HitMarkerState {
    timer: Timer,
}

fn setup(
    mut commands: Commands,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(HitMarkerState {
        timer: Timer::from_seconds(0.1, TimerMode::Once),
    });

    let gun_scene = asset_server.load("Assault Rifle.glb#Scene0");

    commands.insert_resource(AimState {
        aiming: false,
        transition: 0.0,
    });

    commands.insert_resource(Sensitivity {
        horizontal: 0.0018,
        vertical: 0.0015,
    });

    commands.insert_resource(EnemyShootTimer(Timer::from_seconds(
        1.5,
        TimerMode::Repeating,
    )));
    commands.insert_resource(ShootTimer(Timer::from_seconds(0.1, TimerMode::Repeating)));
    commands.insert_resource(RecoilState {
        vertical: 0.0,
        horizontal: 0.0,
    });

    commands.insert_resource(CameraState { pitch: 0.0 });

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_xyz(10.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                margin: UiRect::all(Val::Px(-10.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::rgba(1.0, 1.0, 1.0, 0.0)), // invisible initially
            ..default()
        },
        HitMarker,
    ));

    // Ground
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(10.0, 1.0, 10.0)), // larger ground
            material: materials.add(StandardMaterial { ..default() }),
            transform: Transform::from_xyz(0.0, -0.5, 0.0),
            ..default()
        },
        Collider::cuboid(5.0, 0.5, 5.0),
    ));

    // Player with camera as child
    commands
        .spawn((
            Player,
            RigidBody::Dynamic,
            Collider::capsule_y(0.9, 0.3), // or cuboid if preferred
            LockedAxes::ROTATION_LOCKED,   // prevent falling over
            GravityScale(1.0),             // optional, can tweak fall speed
            Velocity::zero(),              // needed for walking later
            TransformBundle::from(Transform::from_xyz(0.0, 3.0, 5.0)), // start above ground
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Camera3dBundle {
                        transform: Transform::from_xyz(0.0, 1.5, 0.0),
                        ..default()
                    },
                    FpsCamera,
                ))
                .with_children(|camera| {
                    camera.spawn((
                        SpatialBundle {
                            transform: Transform::from_xyz(0.0, 0.0, -0.5), // adjust forward (local -Z)
                            ..default()
                        },
                        GunBarrel,
                    ));

                    // Gun model (attach here)
                    camera.spawn(SceneBundle {
                        scene: gun_scene,
                        transform: Transform {
                            translation: Vec3::new(0.15, -0.1, -0.4), // tweak to position in front of camera
                            // rotation: Quat::from_rotation_y(std::f32::consts::PI),
                            scale: Vec3::splat(0.25), // adjust based on model size
                            ..default()
                        },
                        ..default()
                    });
                });
        });

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                },
                background_color: BackgroundColor(Color::NONE),
                ..default()
            },
            Crosshair,
        ))
        .with_children(|parent| {
            let line_color = BackgroundColor(Color::WHITE);
            let thickness = 2.0;
            let length = 8.0;

            // Vertical line (top)
            parent.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-length),
                    width: Val::Px(thickness),
                    height: Val::Px(length),
                    ..default()
                },
                background_color: line_color,
                ..default()
            });

            // Vertical line (bottom)
            parent.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(thickness),
                    width: Val::Px(thickness),
                    height: Val::Px(length),
                    ..default()
                },
                background_color: line_color,
                ..default()
            });

            // Horizontal line (left)
            parent.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-length),
                    width: Val::Px(length),
                    height: Val::Px(thickness),
                    ..default()
                },
                background_color: line_color,
                ..default()
            });

            // Horizontal line (right)
            parent.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(thickness),
                    width: Val::Px(length),
                    height: Val::Px(thickness),
                    ..default()
                },
                background_color: line_color,
                ..default()
            });
        });

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Capsule3d::new(0.5, 1.0)),
            material: materials.add(Color::rgb(1.0, 0.0, 0.0)),
            transform: Transform::from_xyz(0.0, 1.5, -5.0),
            ..default()
        },
        Enemy,
        RigidBody::Fixed,
        Collider::capsule_y(0.5, 0.6),
        Health {
            current: 100.0,
            max: 100.0,
        },
    ));
    // Lock cursor on startup
    let mut window = windows.single_mut();
    window.cursor.visible = false;
    window.cursor.grab_mode = CursorGrabMode::Locked;
}

fn player_movement(
    keys: Res<ButtonInput<KeyCode>>,
    aim_state: Res<AimState>,
    mut query: Query<(&Transform, &mut Velocity), With<Player>>,
) {
    let (transform, mut velocity) = query.single_mut();
    let mut direction = Vec3::ZERO;

    if keys.pressed(KeyCode::KeyW) {
        direction += transform.forward().as_vec3();
    }
    if keys.pressed(KeyCode::KeyS) {
        direction -= transform.forward().as_vec3();
    }
    if keys.pressed(KeyCode::KeyA) {
        direction -= transform.right().as_vec3();
    }
    if keys.pressed(KeyCode::KeyD) {
        direction += transform.right().as_vec3();
    }

    direction.y = 0.0; // Don't move vertically

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        let ads_speed_factor = 0.4 + 0.6 * (1.0 - aim_state.transition); // 0.4x when fully aiming
        velocity.linvel.x = direction.x * 5.0 * ads_speed_factor;
        velocity.linvel.z = direction.z * 5.0 * ads_speed_factor;
    } else {
        velocity.linvel.x = 0.0;
        velocity.linvel.z = 0.0;
    }

    // Let gravity control y velocity
}

fn mouse_look(
    mut motion_evr: EventReader<MouseMotion>,
    mut player_query: Query<&mut Transform, (With<Player>, Without<FpsCamera>)>,
    mut camera_query: Query<&mut Transform, With<FpsCamera>>,
    mut camera_state: ResMut<CameraState>,
    mut recoil_state: ResMut<RecoilState>,
    sensitivity: Res<Sensitivity>,
    aim_state: Res<AimState>,
) {
    let mut delta = Vec2::ZERO;
    for ev in motion_evr.read() {
        delta += ev.delta;
    }

    delta.x += recoil_state.horizontal * 1000.0;
    delta.y += recoil_state.vertical * 1000.0;

    recoil_state.vertical *= 0.8;
    recoil_state.horizontal *= 0.8;

    if delta == Vec2::ZERO {
        return;
    }

    let mut player_transform = player_query.single_mut();
    let mut camera_transform = camera_query.single_mut();

    let yaw = Quat::from_rotation_y(-delta.x * sensitivity.horizontal);
    player_transform.rotation = yaw * player_transform.rotation;

    camera_state.pitch = (camera_state.pitch - delta.y * sensitivity.vertical).clamp(
        -std::f32::consts::FRAC_PI_2 + 0.01,
        std::f32::consts::FRAC_PI_2 - 0.01,
    );
    camera_transform.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, camera_state.pitch, 0.0);

    // Move camera forward when aiming
    let base_position = Vec3::new(0.0, 1.5, 0.0);
    let ads_offset = Vec3::new(0.0, 1.5, -0.3); // closer to gun barrel

    let new_pos = base_position.lerp(ads_offset, aim_state.transition);
    camera_transform.translation = new_pos;
}

fn shoot_bullet(
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut shoot_timer: ResMut<ShootTimer>,
    mut commands: Commands,
    mut recoil: ResMut<RecoilState>,
    barrel_query: Query<&GlobalTransform, With<GunBarrel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if buttons.pressed(MouseButton::Left) {
        shoot_timer.0.tick(time.delta());

        if shoot_timer.0.finished() {
            let barrel_transform = barrel_query.single();
            let position = barrel_transform.translation();
            let direction = barrel_transform.forward();

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Sphere::new(0.05)),
                    material: materials.add(Color::rgb(1.0, 0.8, 0.1)),
                    transform: Transform::from_translation(position),
                    ..default()
                },
                Bullet { lifetime: 3.0 },
                Shooter::Player,
                RigidBody::Dynamic,
                Collider::ball(0.05),
                Velocity::linear(direction * 100.0),
                ActiveEvents::COLLISION_EVENTS,
            ));

            let tick = shoot_timer.0.times_finished_this_tick();
            let horizontal_pattern = (tick as f32).sin() * 0.002;
            recoil.vertical -= 0.012;
            recoil.horizontal += horizontal_pattern;

            recoil.vertical = recoil.vertical.clamp(-0.03, 0.03);
            recoil.horizontal = recoil.horizontal.clamp(-0.03, 0.03);
        }
    } else {
        // Reset the timer when not holding
        shoot_timer.0.reset();
    }
}

fn update_bullets(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Bullet)>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();

    for (entity, mut bullet) in &mut query {
        bullet.lifetime -= delta;
        if bullet.lifetime <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn bullet_collision(
    mut collision_events: EventReader<CollisionEvent>,
    mut commands: Commands,
    bullet_query: Query<(Entity, &Shooter), With<Bullet>>,
    mut enemy_query: Query<(Entity, &mut Health), With<Enemy>>,
    player_query: Query<Entity, With<Player>>,
    mut hit_marker_state: ResMut<HitMarkerState>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(e1, e2, _) = event {
            let (bullet_entity, bullet_shooter) = if let Ok(result) = bullet_query.get(*e1) {
                result
            } else if let Ok(result) = bullet_query.get(*e2) {
                result
            } else {
                continue;
            };

            let other_entity = if bullet_entity == *e1 { *e2 } else { *e1 };

            match bullet_shooter {
                Shooter::Player if player_query.get(other_entity).is_ok() => continue,
                Shooter::Enemy if enemy_query.get_mut(other_entity).is_err() => continue,
                _ => {}
            }

            if let Shooter::Player = bullet_shooter {
                if let Ok((enemy_entity, mut health)) = enemy_query.get_mut(other_entity) {
                    health.current -= 25.0;
                    hit_marker_state.timer.reset();

                    if health.current <= 0.0 {
                        info!("Enemy {:?} was killed!", enemy_entity);
                        commands.entity(enemy_entity).despawn_recursive();
                    }
                }
            }

            commands.entity(bullet_entity).despawn();
        }
    }
}

fn enemy_shoots(
    time: Res<Time>,
    mut timer: ResMut<EnemyShootTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemy_query: Query<&GlobalTransform, With<Enemy>>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.finished() {
        return;
    }

    let player_transform = player_query.single();
    for enemy_transform in enemy_query.iter() {
        let position = enemy_transform.translation();
        let direction = (player_transform.translation() - position).normalize();

        info!("Enemy shooting bullet towards player at {:?}", position);
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Sphere::new(0.05)),
                material: materials.add(Color::rgb(1.0, 0.2, 0.2)),
                transform: Transform::from_translation(position),
                ..default()
            },
            Bullet { lifetime: 5.0 },
            Shooter::Enemy,
            RigidBody::Dynamic,
            Collider::ball(0.05),
            Velocity::linear(direction * 100.0),
            ActiveEvents::COLLISION_EVENTS,
        ));
    }
}

fn update_aiming_state(
    buttons: Res<ButtonInput<MouseButton>>,
    mut aim_state: ResMut<AimState>,
    time: Res<Time>,
) {
    aim_state.aiming = buttons.pressed(MouseButton::Right);
    let target = if aim_state.aiming { 1.0 } else { 0.0 };

    // Smooth transition
    let speed = 5.0;
    aim_state.transition += (target - aim_state.transition) * speed * time.delta_seconds();
}

fn update_camera_fov(mut query: Query<&mut Projection, With<FpsCamera>>, aim_state: Res<AimState>) {
    let mut projection = query.single_mut();

    if let Projection::Perspective(perspective) = &mut *projection {
        let base_fov = std::f32::consts::FRAC_PI_3; // 60 degrees
        let ads_fov = std::f32::consts::FRAC_PI_6; // 30 degrees

        perspective.fov = base_fov.lerp(ads_fov, aim_state.transition);
    }
}

fn update_hit_marker(
    time: Res<Time>,
    mut state: ResMut<HitMarkerState>,
    mut query: Query<&mut BackgroundColor, With<HitMarker>>,
) {
    state.timer.tick(time.delta());
    let alpha = if state.timer.finished() { 0.0 } else { 1.0 };

    if let Ok(mut bg_color) = query.get_single_mut() {
        // Assume we're using white color with varying alpha
        bg_color.0 = Color::rgba(1.0, 1.0, 1.0, alpha);
    }
}
