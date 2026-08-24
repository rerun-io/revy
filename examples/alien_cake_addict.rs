//! Example from <https://github.com/bevyengine/bevy/blob/v0.19.0/examples/showcase/alien_cake_addict.rs>
//! with minimal changes to inject revy.
//!
//! This is part of the Bevy project and licensed separately from Revy under MIT & Apache-2.0.
//! For details see <https://github.com/bevyengine/bevy/tree/v0.19.0?tab=readme-ov-file#license>
//!
//! ------------------------------------------------------------------------------------------------
//!
//! Eat the cakes. Eat them all. An example 3D game.

#![allow(clippy::unwrap_used)]
#![allow(clippy::needless_pass_by_value)]
#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::missing_assert_message)]
#![allow(clippy::disallowed_methods)]
#![allow(clippy::str_to_string)]
#![allow(clippy::type_complexity)]
#![allow(clippy::explicit_iter_loop)]

use std::f32::consts::PI;

use bevy::prelude::*;

use chacha20::ChaCha8Rng;
use rand::{RngExt as _, SeedableRng as _};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

#[derive(Resource)]
struct BonusSpawnTimer(Timer);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // ==== Instantiating the Rerun plugin ===========================================
        //
        // This is the only modification that was applied to this example.
        //
        // This will start a Rerun Viewer in the background and stream the recording data to it.
        // Check out the `RecordingStreamBuilder` (<https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html>)
        // docs for other options (saving to file, connecting to a remote viewer, etc).
        .add_plugins({
            let rec = revy::RecordingStreamBuilder::new("alien_cake_addict")
                .spawn()
                .unwrap();
            revy::RerunPlugin { rec }
        })
        // ===============================================================================
        .register_type::<UserInput>()
        .init_resource::<Game>()
        .insert_resource(BonusSpawnTimer(Timer::from_seconds(
            5.0,
            TimerMode::Repeating,
        )))
        .init_state::<GameState>()
        .add_systems(Startup, setup_cameras)
        .add_systems(OnEnter(GameState::Playing), setup)
        .add_systems(
            Update,
            (
                capture_user_input,
                move_player.after(capture_user_input),
                focus_camera,
                rotate_bonus,
                scoreboard_system,
                spawn_bonus,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnEnter(GameState::GameOver), display_score)
        .add_systems(
            Update,
            game_over_keyboard.run_if(in_state(GameState::GameOver)),
        )
        .run();
}

struct Cell {
    height: f32,
}

#[derive(Default)]
struct Player {
    entity: Option<Entity>,
    i: usize,
    j: usize,
    move_cooldown: Timer,
}

#[derive(Default)]
struct Bonus {
    entity: Option<Entity>,
    i: usize,
    j: usize,
    handle: Handle<WorldAsset>,
}

#[derive(Resource, Default)]
struct Game {
    board: Vec<Vec<Cell>>,
    player: Player,
    bonus: Bonus,
    score: i32,
    cake_eaten: u32,
    camera_should_focus: Vec3,
    camera_is_focus: Vec3,
}

#[derive(Resource, Deref, DerefMut)]
struct Random(ChaCha8Rng);

/// A per-frame snapshot of the raw keyboard state that's relevant to gameplay.
///
/// `capture_user_input` writes this every frame from `ButtonInput<KeyCode>`, and `move_player`
/// (which runs after it) reads *this* instead of polling `ButtonInput<KeyCode>` directly. From
/// the player's perspective nothing changes -- movement still responds to the same keys on the
/// same frame. What changes is that raw input is now itself a `Component`, which means a
/// debugger/inspector walking the ECS (like the Rerun viewer, via `revy`) can see exactly what
/// input the game acted on each frame, not just the resulting player position -- and, being on
/// its own dedicated entity rather than `ButtonInput<KeyCode>` (a `Resource`, invisible to
/// `revy`) or a field tacked onto the player, it shows up as its own clearly separate thing in
/// the entity tree instead of being lost among the (much more numerous) board tiles.
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
#[reflect(Component)]
struct UserInput {
    arrow_up: bool,
    arrow_down: bool,
    arrow_left: bool,
    arrow_right: bool,
}

const BOARD_SIZE_I: usize = 14;
const BOARD_SIZE_J: usize = 21;

const RESET_FOCUS: [f32; 3] = [
    BOARD_SIZE_I as f32 / 2.0,
    0.0,
    BOARD_SIZE_J as f32 / 2.0 - 0.5,
];

fn setup_cameras(mut commands: Commands, mut game: ResMut<Game>) {
    game.camera_should_focus = Vec3::from(RESET_FOCUS);
    game.camera_is_focus = game.camera_should_focus;
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            -(BOARD_SIZE_I as f32 / 2.0),
            2.0 * BOARD_SIZE_J as f32 / 3.0,
            BOARD_SIZE_J as f32 / 2.0 - 0.5,
        )
        .looking_at(game.camera_is_focus, Vec3::Y),
    ));
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut game: ResMut<Game>) {
    let mut rng = if std::env::var("GITHUB_ACTIONS") == Ok("true".to_string()) {
        // We're seeding the PRNG here to make this example deterministic for testing purposes.
        // This isn't strictly required in practical use unless you need your app to be deterministic.
        ChaCha8Rng::seed_from_u64(19878367467713)
    } else {
        rand::make_rng()
    };

    // reset the game state
    game.cake_eaten = 0;
    game.score = 0;
    game.player.i = BOARD_SIZE_I / 2;
    game.player.j = BOARD_SIZE_J / 2;
    game.player.move_cooldown = Timer::from_seconds(0.3, TimerMode::Once);

    commands.spawn((
        DespawnOnExit(GameState::Playing),
        Name::new("input"),
        UserInput::default(),
    ));

    commands.spawn((
        DespawnOnExit(GameState::Playing),
        PointLight {
            intensity: 2_000_000.0,
            shadow_maps_enabled: true,
            range: 30.0,
            ..default()
        },
        Transform::from_xyz(4.0, 10.0, 4.0),
    ));

    // spawn the game board
    //
    // All tiles are spawned as children of a single `board` entity (rather than directly under
    // the scene root) purely so that debuggers/inspectors -- e.g. the Rerun viewer via `revy` --
    // can show the (potentially large) grid of tiles as one collapsible group instead of a flat
    // list of hundreds of same-looking, arbitrarily-named entities alongside the player, camera,
    // etc. `DespawnOnExit` only needs to live on `board` itself: despawning an entity in Bevy is
    // recursive by default, so its tile children go with it.
    let cell_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/AlienCake/tile.glb"));
    let board = commands
        .spawn((
            DespawnOnExit(GameState::Playing),
            Name::new("board"),
            Transform::IDENTITY,
            Visibility::default(),
        ))
        .id();
    game.board = (0..BOARD_SIZE_J)
        .map(|j| {
            (0..BOARD_SIZE_I)
                .map(|i| {
                    let height = rng.random_range(-0.1..0.1);
                    commands.spawn((
                        ChildOf(board),
                        Name::new(format!("tile_{i}_{j}")),
                        Transform::from_xyz(i as f32, height - 0.2, j as f32),
                        WorldAssetRoot(cell_scene.clone()),
                    ));
                    Cell { height }
                })
                .collect()
        })
        .collect();

    // spawn the game character
    game.player.entity = Some(
        commands
            .spawn((
                DespawnOnExit(GameState::Playing),
                Transform {
                    translation: Vec3::new(
                        game.player.i as f32,
                        game.board[game.player.j][game.player.i].height,
                        game.player.j as f32,
                    ),
                    rotation: Quat::from_rotation_y(-PI / 2.),
                    ..default()
                },
                WorldAssetRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("models/AlienCake/alien.glb")),
                ),
            ))
            .id(),
    );

    // load the scene for the cake
    game.bonus.handle =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/AlienCake/cakeBirthday.glb"));

    // scoreboard
    commands.spawn((
        DespawnOnExit(GameState::Playing),
        Text::new("Score:"),
        TextFont {
            font_size: FontSize::Px(33.0),
            ..default()
        },
        TextColor(Color::srgb(0.5, 0.5, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(5),
            left: px(5),
            ..default()
        },
    ));

    commands.insert_resource(Random(rng));
}

/// Copies the raw keyboard state relevant to gameplay into the `UserInput` component, so that
/// `move_player` (and anything watching the recording) sees input as ECS data rather than
/// reaching into `ButtonInput<KeyCode>` directly. Must run before `move_player`.
fn capture_user_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut input: Single<&mut UserInput>,
) {
    **input = UserInput {
        arrow_up: keyboard_input.pressed(KeyCode::ArrowUp),
        arrow_down: keyboard_input.pressed(KeyCode::ArrowDown),
        arrow_left: keyboard_input.pressed(KeyCode::ArrowLeft),
        arrow_right: keyboard_input.pressed(KeyCode::ArrowRight),
    };
}

// control the game character
fn move_player(
    mut commands: Commands,
    input: Single<&UserInput>,
    mut game: ResMut<Game>,
    mut transforms: Query<&mut Transform>,
    time: Res<Time>,
) {
    if game.player.move_cooldown.tick(time.delta()).is_finished() {
        let mut moved = false;
        let mut rotation = 0.0;

        if input.arrow_up {
            if game.player.i < BOARD_SIZE_I - 1 {
                game.player.i += 1;
            }
            rotation = -PI / 2.;
            moved = true;
        }
        if input.arrow_down {
            if game.player.i > 0 {
                game.player.i -= 1;
            }
            rotation = PI / 2.;
            moved = true;
        }
        if input.arrow_right {
            if game.player.j < BOARD_SIZE_J - 1 {
                game.player.j += 1;
            }
            rotation = PI;
            moved = true;
        }
        if input.arrow_left {
            if game.player.j > 0 {
                game.player.j -= 1;
            }
            rotation = 0.0;
            moved = true;
        }

        // move on the board
        if moved {
            game.player.move_cooldown.reset();
            *transforms.get_mut(game.player.entity.unwrap()).unwrap() = Transform {
                translation: Vec3::new(
                    game.player.i as f32,
                    game.board[game.player.j][game.player.i].height,
                    game.player.j as f32,
                ),
                rotation: Quat::from_rotation_y(rotation),
                ..default()
            };
        }
    }

    // eat the cake!
    if let Some(entity) = game.bonus.entity
        && game.player.i == game.bonus.i
        && game.player.j == game.bonus.j
    {
        game.score += 2;
        game.cake_eaten += 1;
        commands.entity(entity).despawn();
        game.bonus.entity = None;
    }
}

// change the focus of the camera
fn focus_camera(
    time: Res<Time>,
    mut game: ResMut<Game>,
    mut transforms: ParamSet<(Query<&mut Transform, With<Camera3d>>, Query<&Transform>)>,
) {
    const SPEED: f32 = 2.0;
    // if there is both a player and a bonus, target the mid-point of them
    if let (Some(player_entity), Some(bonus_entity)) = (game.player.entity, game.bonus.entity) {
        let transform_query = transforms.p1();
        if let (Ok(player_transform), Ok(bonus_transform)) = (
            transform_query.get(player_entity),
            transform_query.get(bonus_entity),
        ) {
            game.camera_should_focus = player_transform
                .translation
                .lerp(bonus_transform.translation, 0.5);
        }
        // otherwise, if there is only a player, target the player
    } else if let Some(player_entity) = game.player.entity {
        if let Ok(player_transform) = transforms.p1().get(player_entity) {
            game.camera_should_focus = player_transform.translation;
        }
        // otherwise, target the middle
    } else {
        game.camera_should_focus = Vec3::from(RESET_FOCUS);
    }
    // calculate the camera motion based on the difference between where the camera is looking
    // and where it should be looking; the greater the distance, the faster the motion;
    // smooth out the camera movement using the frame time
    let mut camera_motion = game.camera_should_focus - game.camera_is_focus;
    if camera_motion.length() > 0.2 {
        camera_motion *= SPEED * time.delta_secs();
        // set the new camera's actual focus
        game.camera_is_focus += camera_motion;
    }
    // look at that new camera's actual focus
    for mut transform in transforms.p0().iter_mut() {
        *transform = transform.looking_at(game.camera_is_focus, Vec3::Y);
    }
}

// despawn the bonus if there is one, then spawn a new one at a random location
fn spawn_bonus(
    time: Res<Time>,
    mut timer: ResMut<BonusSpawnTimer>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut rng: ResMut<Random>,
) {
    // make sure we wait enough time before spawning the next cake
    if !timer.0.tick(time.delta()).is_finished() {
        return;
    }

    if let Some(entity) = game.bonus.entity {
        game.score -= 3;
        commands.entity(entity).despawn();
        game.bonus.entity = None;
        if game.score <= -5 {
            next_state.set(GameState::GameOver);
            return;
        }
    }

    // ensure bonus doesn't spawn on the player
    loop {
        game.bonus.i = rng.random_range(0..BOARD_SIZE_I);
        game.bonus.j = rng.random_range(0..BOARD_SIZE_J);
        if game.bonus.i != game.player.i || game.bonus.j != game.player.j {
            break;
        }
    }
    game.bonus.entity = Some(
        commands
            .spawn((
                DespawnOnExit(GameState::Playing),
                Transform::from_xyz(
                    game.bonus.i as f32,
                    game.board[game.bonus.j][game.bonus.i].height + 0.2,
                    game.bonus.j as f32,
                ),
                WorldAssetRoot(game.bonus.handle.clone()),
                children![(
                    PointLight {
                        color: Color::srgb(1.0, 1.0, 0.0),
                        intensity: 500_000.0,
                        range: 10.0,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 2.0, 0.0),
                )],
            ))
            .id(),
    );
}

// let the cake turn on itself
fn rotate_bonus(game: Res<Game>, time: Res<Time>, mut transforms: Query<&mut Transform>) {
    if let Some(entity) = game.bonus.entity
        && let Ok(mut cake_transform) = transforms.get_mut(entity)
    {
        cake_transform.rotate_y(time.delta_secs());
        cake_transform.scale =
            Vec3::splat(1.0 + (game.score as f32 / 10.0 * ops::sin(time.elapsed_secs())).abs());
    }
}

// update the score displayed during the game
fn scoreboard_system(game: Res<Game>, mut display: Single<&mut Text>) {
    display.0 = format!("Sugar Rush: {}", game.score);
}

// restart the game when pressing spacebar
fn game_over_keyboard(
    mut next_state: ResMut<NextState<GameState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
    }
}

// display the number of cake eaten before losing
fn display_score(mut commands: Commands, game: Res<Game>) {
    commands.spawn((
        DespawnOnExit(GameState::GameOver),
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Text::new(format!("Cake eaten: {}", game.cake_eaten)),
            TextFont {
                font_size: FontSize::Px(67.0),
                ..default()
            },
            TextColor(Color::srgb(0.5, 0.5, 1.0)),
        )],
    ));
}
