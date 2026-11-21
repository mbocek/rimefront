use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::ScheduleSystem;
use bevy::prelude::*;

use crate::{
    FramePhase, StartupPhase, abilities, ambience, animation, announcer, assets, battle_noise,
    battlefield, camera, campaigns, capture, combat, control_groups, crystal_models,
    damage_feedback, economy, effects, field_tips, flat_scenery, fog_view, frame_pacing, graphics,
    interpolation, lobby, local_view, main_menu, map, map_editor, map_edits, match_flow, menu,
    minimap, motion_material, network, network_hud, orders, overlay, perf_overlay, placement,
    queue_bar, replay, savegame, settings, sorties, sound, structure_tools, terrain_material,
    transport, tutorial, ui, unit_voices, updates, wall_joints, weather,
};

/// Draws what the simulation moves part way between ticks.
pub struct InterpolationPlugin;

impl Plugin for InterpolationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedFirst, interpolation::restore_simulated_transforms)
            .add_systems(FixedLast, interpolation::record_simulated_transforms)
            .add_systems(
                RunFixedMainLoop,
                interpolation::draw_between_ticks
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

/// The battle on screen: interface, orders, overlays, models, scenery and the battlefield's marks.
pub struct BattleViewPlugin;

impl Plugin for BattleViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<camera::Cursor>()
            .init_resource::<camera::ScriptedPointer>()
            .init_resource::<orders::DragSelection>()
            .init_resource::<orders::OrderMarker>()
            .init_resource::<minimap::MinimapOrder>()
            .init_resource::<crystal_models::CrystalModelLibrary>()
            .init_resource::<animation::AnimationLibrary>()
            .init_gizmo_group::<overlay::HealthBarGizmos>()
            .add_systems(
                Update,
                (
                    animation::link_rotor_nodes,
                    animation::spin_rotors,
                    sorties::find_landing_gear,
                    sorties::fold_landing_gear,
                )
                    .chain()
                    .in_set(FramePhase::Presentation),
            )
            .add_systems(
                Update,
                (
                    tutorial::follow_tutorial,
                    tutorial::refresh_tutorial_panel,
                    field_tips::offer_field_tips.run_if(in_state(match_flow::GameState::Playing)),
                )
                    .chain()
                    .in_set(FramePhase::Presentation),
            )
            .add_systems(
                Update,
                (
                    flat_scenery::merge_scenery_models,
                    flat_scenery::dress_flat_scenery,
                    flat_scenery::follow_wind_setting,
                    (weather::follow_the_weather, weather::announce_weather).chain(),
                    crystal_models::merge_crystal_models,
                    crystal_models::dress_crystals,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    battlefield::scorch_the_ground,
                    battlefield::fade_craters,
                    battlefield::leave_wrecks,
                    battlefield::char_wrecks,
                    wall_joints::join_walls,
                    battlefield::smoulder_and_sink_wrecks,
                    battlefield::draw_rain,
                )
                    .chain()
                    .in_set(FramePhase::Presentation),
            )
            .init_resource::<structure_tools::StructureTool>()
            .add_systems(
                OnEnter(match_flow::GameState::Playing),
                (
                    local_view::prepare_spectator_view,
                    camera::focus_on_player_base,
                    map::spawn_terrain_visuals,
                    fog_view::shroud_unexplored_scenery,
                    minimap::create_minimap,
                    ui::build_interface,
                    structure_tools::prepare_structure_tools,
                    tutorial::start_tutorial,
                    field_tips::spawn_tip_panel,
                    weather::begin_weather,
                    replay::start_recording,
                    orders::spawn_selection_box,
                    ambience::light_crystal_fields,
                    ambience::index_trampleable_vegetation,
                    menu::close_menu,
                )
                    .chain()
                    .after(match_flow::deploy_level_forces)
                    .before(savegame::restore_pending_load),
            )
            .add_systems(
                Update,
                (ui::highlight_outcome_buttons, ui::refresh_build_tooltip)
                    .in_set(FramePhase::Presentation),
            )
            .add_systems(
                Update,
                (
                    camera::track_cursor,
                    camera::control_camera,
                    control_groups::handle_group_keys,
                    graphics::cycle_quality_preset,
                    perf_overlay::toggle_performance_overlay,
                    menu::toggle_menu,
                    menu::pause_while_menu_open,
                    savegame::quick_save_keys,
                    menu::rebuild_menu_page,
                    menu::refresh_menu,
                    ui::toggle_controls_help,
                    orders::select_units,
                    orders::issue_orders,
                    placement::place_structure,
                    placement::preview_wall_drag,
                    structure_tools::apply_structure_tool,
                    structure_tools::switch_tools_by_keyboard,
                )
                    .chain()
                    .in_set(FramePhase::Input),
            )
            .add_systems(
                Update,
                (
                    structure_tools::label_tool_buttons,
                    ui::fill_controls_help,
                    menu::capture_key_binding,
                )
                    .in_set(FramePhase::Presentation),
            )
            .add_systems(
                Update,
                (
                    (
                        economy::link_ore_load_nodes,
                        economy::show_harvester_load,
                        combat::link_turret_nodes,
                        combat::rotate_turret_nodes,
                        ambience::trample_vegetation,
                        abilities::show_own_mines,
                        ambience::emit_crystal_sparkles,
                        ambience::pulse_crystal_glow,
                        effects::animate_sparkles,
                        transport::deselect_passengers,
                        effects::dress_projectiles,
                        effects::show_blasts,
                        effects::animate_blasts,
                        damage_feedback::spawn_damage_numbers,
                        damage_feedback::float_damage_numbers,
                        damage_feedback::emit_damage_smoke,
                        terrain_material::assemble_terrain_material,
                        terrain_material::fit_terrain_to_level,
                        terrain_material::animate_water,
                    )
                        .chain(),
                    (
                        graphics::apply_graphics_settings,
                        settings::apply_window_settings,
                        settings::apply_game_speed,
                        settings::persist_settings,
                        graphics::strip_fine_decor_shadows,
                        graphics::cull_distant_decor,
                        animation::build_animation_graphs,
                        animation::attach_animation_players,
                        animation::animate_soldiers,
                        animation::animate_vehicles,
                        animation::animate_corpses,
                    )
                        .chain(),
                    (
                        (
                            overlay::draw_overlays,
                            overlay::draw_planned_routes,
                            overlay::draw_weapon_ranges,
                        ),
                        structure_tools::draw_repair_markers,
                        structure_tools::refresh_tool_buttons,
                        structure_tools::refresh_tool_hint,
                        ui::refresh_build_tiles,
                        ui::refresh_resources,
                        ui::refresh_info_panel,
                        queue_bar::rebuild_queue_bar,
                        control_groups::refresh_group_bar,
                        perf_overlay::refresh_performance_overlay,
                        queue_bar::animate_queue_progress,
                        (ui::announce_mission_messages, ui::refresh_notifications).chain(),
                        (ui::refresh_objective, ui::refresh_match_time),
                        terrain_material::dress_terrain_surfaces,
                        fog_view::paint_fog_texture,
                        fog_view::reveal_explored_scenery,
                        fog_view::show_seen_enemies,
                        fog_view::show_seen_projectiles,
                        minimap::redraw_minimap,
                        ui::show_outcome,
                    )
                        .chain(),
                )
                    .chain()
                    .in_set(FramePhase::Presentation),
            );
    }
}

/// The main menu, the in-game menu, the network lobby and update news.
pub struct MenusPlugin;

impl Plugin for MenusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<campaigns::CampaignLibrary>()
            .init_resource::<menu::MenuState>()
            .init_resource::<updates::Updates>()
            .init_resource::<lobby::NetLobby>()
            .init_resource::<network_hud::BattleChat>()
            .add_systems(
                PostStartup,
                (match_flow::finish_booting, updates::check_for_updates),
            )
            .add_systems(
                OnEnter(match_flow::GameState::MainMenu),
                (
                    match_flow::unpause,
                    lobby::end_network_battle,
                    campaigns::reload_campaigns,
                    main_menu::build_main_menu,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    updates::receive_update_news,
                    main_menu::type_on_network_page,
                    lobby::run_lobby,
                    main_menu::step_back_on_escape,
                    main_menu::rebuild_screen,
                    main_menu::refresh_main_menu,
                    main_menu::show_network_status,
                    main_menu::refresh_update_banner,
                    menu::pause_while_menu_open,
                    menu::rebuild_menu_page,
                    menu::refresh_menu,
                )
                    .chain()
                    .run_if(in_state(match_flow::GameState::MainMenu)),
            );
    }
}

/// Sound effects, music, battle noise, the announcer and unit voices.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<sound::MusicDirector>()
            .init_resource::<sound::BattleFury>()
            .add_observer(sound::click_feedback)
            .add_systems(
                OnEnter(match_flow::GameState::Playing),
                sound::count_match_start,
            )
            .add_systems(
                Update,
                (
                    sound::play_sound_cues,
                    sound::direct_music,
                    sound::apply_master_volume,
                    battle_noise::mix_battle_noise,
                    announcer::announce,
                    unit_voices::answer_the_player,
                )
                    .after(FramePhase::Presentation),
            );
    }
}

/// Settings, graphics quality, frame pacing, diagnostics and the custom materials.
pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(settings::load_settings())
            .add_systems(Last, frame_pacing::pace_frames)
            .add_systems(Update, graphics::fit_shadows_to_zoom)
            .add_systems(Update, settings::apply_language)
            .add_systems(
                Update,
                settings::persist_settings.run_if(in_state(match_flow::GameState::MainMenu)),
            )
            .add_plugins((
                bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
                bevy::diagnostic::SystemInformationDiagnosticsPlugin,
                bevy::diagnostic::EntityCountDiagnosticsPlugin::default(),
                bevy::render::diagnostic::RenderDiagnosticsPlugin,
                capture::CapturePlugin,
                MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
                MaterialPlugin::<terrain_material::WaterMaterial>::default(),
                MaterialPlugin::<motion_material::MovingMaterial>::default(),
            ));
    }
}

/// Recording battles and watching them again.
pub struct ReplayPlugin;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                replay::record_replay_events,
                replay::record_replay_frame,
                replay::save_replay_when_decided,
            )
                .chain()
                .in_set(FramePhase::Presentation),
        )
        .add_systems(
            OnExit(match_flow::GameState::Playing),
            replay::save_abandoned_replay,
        )
        .add_systems(
            OnEnter(match_flow::GameState::Replay),
            (
                match_flow::unpause,
                replay::clear_the_fog,
                map::generate_map,
                map_edits::apply_map_edits,
                map::spawn_terrain_visuals,
                camera::focus_on_player_base,
                weather::begin_weather,
                replay::spawn_replay_hud,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                world_view(),
                replay::control_replay,
                replay::play_replay,
                wall_joints::join_walls,
                replay::fly_replay_shots,
                effects::animate_blasts,
                animation::attach_animation_players,
                animation::animate_replay_actors,
                animation::link_rotor_nodes,
                animation::spin_rotors,
                replay::refresh_replay_hud,
            )
                .chain()
                .run_if(in_state(match_flow::GameState::Replay)),
        )
        .add_systems(OnExit(match_flow::GameState::Replay), replay::stop_watching)
        .add_systems(Last, replay::save_replay_on_exit);
    }
}

/// Repainting a level's map by hand.
pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(match_flow::GameState::MapEditor),
            (
                match_flow::unpause,
                match_flow::reset_match_state,
                replay::clear_the_fog,
                map::generate_map,
                map_edits::apply_map_edits,
                map::spawn_terrain_visuals,
                camera::focus_on_player_base,
                weather::begin_weather,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                world_view(),
                animation::attach_animation_players,
                map_editor::choose_brush_by_key,
                map_editor::paint_under_cursor,
                map_editor::resoil_after_stroke,
                map_editor::draw_brush_outline,
                map_editor::mark_base_sites,
                map_editor::rebuild_map_editor_panel,
                map_editor::leave_on_escape,
            )
                .chain()
                .run_if(in_state(match_flow::GameState::MapEditor)),
        )
        .add_systems(
            PreUpdate,
            map_editor::type_level_text
                .after(bevy::input::InputSystems)
                .run_if(in_state(match_flow::GameState::MapEditor)),
        );
    }
}

/// Loads models, textures, sounds and fonts at startup.
pub struct AssetLoadingPlugin;

impl Plugin for AssetLoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                assets::load_models,
                animation::load_animation_sources,
                terrain_material::load_terrain_textures,
                effects::create_effect_assets,
                placement::create_ghost_materials,
                camera::setup_scene,
                overlay::configure_gizmos,
                ui::load_fonts,
                fog_view::create_fog_texture,
                sound::load_sounds,
                flat_scenery::load_flat_scenery_sources,
                crystal_models::load_crystal_models,
                battlefield::create_battlefield_assets,
                abilities::create_mine_look,
                sound::place_ears,
                battle_noise::start_battle_noise,
                announcer::load_recordings,
                unit_voices::load_unit_voices,
            )
                .chain()
                .in_set(StartupPhase::Assets),
        );
    }
}

/// Catching up, chat and trouble reports during a network battle.
pub struct NetworkBattlePlugin;

impl Plugin for NetworkBattlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                network::report_session_trouble,
                network::catch_up_with_the_battle.run_if(in_state(match_flow::GameState::Playing)),
                network_hud::show_chat.run_if(in_state(match_flow::GameState::Playing)),
                main_menu::remember_won_missions.run_if(in_state(match_flow::GameState::Playing)),
                network_hud::refresh_network_hud.run_if(in_state(match_flow::GameState::Playing)),
            ),
        )
        .add_systems(
            PreUpdate,
            network_hud::type_battle_chat
                .after(bevy::input::InputSystems)
                .run_if(in_state(match_flow::GameState::Playing)),
        );
    }
}

/// The camera, terrain, water, fog and rain that every view of a battlefield shares.
fn world_view() -> ScheduleConfigs<ScheduleSystem> {
    (
        camera::track_cursor,
        camera::control_camera,
        fog_view::paint_fog_texture,
        terrain_material::assemble_terrain_material,
        terrain_material::fit_terrain_to_level,
        terrain_material::dress_terrain_surfaces,
        terrain_material::animate_water,
        battlefield::draw_rain,
    )
        .chain()
}
