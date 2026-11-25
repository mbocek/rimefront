use bevy::ecs::schedule::ScheduleConfigs;
use bevy::ecs::system::ScheduleSystem;
use bevy::prelude::*;

use crate::{
    FramePhase, StartupPhase, app::capture, app::catching_up, app::frame_pacing, app::match_flow,
    app::replay, app::savegame, app::settings, app::updates, audio::announcer, audio::battle_noise,
    audio::sound, audio::unit_voices, missions::campaigns, net::lobby, net::network,
    sim::transport, ui::control_groups, ui::field_tips, ui::main_menu, ui::map_editor, ui::menu,
    ui::minimap, ui::network_hud, ui::orders, ui::perf_overlay, ui::placement, ui::queue_bar,
    ui::sidebar, ui::structure_tools, ui::support_tools, ui::tutorial, view::ambience,
    view::animation, view::assets, view::battlefield, view::camera, view::crystal_models,
    view::damage_feedback, view::dressing, view::effects, view::flat_scenery, view::fog_view,
    view::graphics, view::interpolation, view::local_view, view::motion_material, view::overlay,
    view::scenery_view, view::sky, view::terrain_chunks, view::terrain_material, view::wall_joints,
    world::map, world::map_edits,
};

/// Draws what the simulation moves part way between ticks.
pub struct InterpolationPlugin;

impl Plugin for InterpolationPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(interpolation::follow_new_unit)
            .add_observer(interpolation::follow_new_projectile)
            .add_systems(FixedFirst, interpolation::restore_simulated_transforms)
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
        app.add_observer(dressing::dress_new_unit)
            .add_observer(dressing::dress_new_building)
            .add_observer(dressing::hide_new_mine)
            .add_observer(crystal_models::show_new_crystal)
            .init_resource::<camera::Cursor>()
            .init_resource::<camera::ScriptedPointer>()
            .init_resource::<orders::DragSelection>()
            .init_resource::<orders::OrderMarker>()
            .init_resource::<minimap::MinimapOrder>()
            .init_resource::<crystal_models::CrystalModelLibrary>()
            .init_resource::<animation::AnimationLibrary>()
            .add_systems(
                Update,
                (
                    terrain_chunks::follow_widened_ground,
                    minimap::follow_widened_world,
                    ambience::light_widened_fields,
                )
                    .run_if(in_state(match_flow::GameState::Playing)),
            )
            .init_resource::<placement::Placement>()
            .init_resource::<control_groups::ControlGroups>()
            .init_gizmo_group::<overlay::HealthBarGizmos>()
            .add_systems(
                Update,
                (
                    animation::link_rotor_nodes,
                    animation::spin_rotors,
                    dressing::find_landing_gear,
                    dressing::fold_landing_gear,
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
                    (sky::follow_the_weather, sky::announce_weather).chain(),
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
            .init_resource::<scenery_view::SceneryShown>()
            .init_resource::<support_tools::SupportTool>()
            .add_systems(
                OnEnter(match_flow::GameState::Playing),
                (
                    placement::start_without_placement,
                    control_groups::start_without_groups,
                    local_view::prepare_spectator_view,
                    camera::focus_on_player_base,
                    terrain_chunks::spawn_terrain_visuals,
                    minimap::create_minimap,
                    sidebar::build_interface,
                    structure_tools::prepare_structure_tools,
                    support_tools::prepare_support_tool,
                    tutorial::start_tutorial,
                    field_tips::spawn_tip_panel,
                    sky::begin_weather,
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
                (
                    sidebar::highlight_outcome_buttons,
                    sidebar::refresh_build_tooltip,
                )
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
                    sidebar::toggle_controls_help,
                    orders::select_units,
                    orders::issue_orders,
                    placement::place_structure,
                    placement::preview_wall_drag,
                    structure_tools::apply_structure_tool,
                    structure_tools::switch_tools_by_keyboard,
                    support_tools::aim_support_by_keyboard,
                    support_tools::call_aimed_support,
                )
                    .chain()
                    .in_set(FramePhase::Input),
            )
            .add_systems(
                Update,
                (
                    structure_tools::label_tool_buttons,
                    sidebar::fill_controls_help,
                    menu::capture_key_binding,
                )
                    .in_set(FramePhase::Presentation),
            )
            .add_systems(
                Update,
                (
                    (
                        dressing::link_ore_load_nodes,
                        dressing::show_harvester_load,
                        dressing::link_turret_nodes,
                        dressing::rotate_turret_nodes,
                        dressing::lay_corpses,
                        ambience::trample_vegetation,
                        dressing::show_own_mines,
                        ambience::emit_crystal_sparkles,
                        ambience::pulse_crystal_glow,
                        effects::animate_sparkles,
                        transport::deselect_passengers,
                        (
                            effects::dress_projectiles,
                            effects::dress_shown_shots,
                            effects::fly_shown_shots,
                        ),
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
                        (
                            structure_tools::draw_repair_markers,
                            support_tools::draw_support_aim,
                            support_tools::refresh_support_buttons,
                        ),
                        structure_tools::refresh_tool_buttons,
                        structure_tools::refresh_tool_hint,
                        sidebar::refresh_build_tiles,
                        sidebar::refresh_resources,
                        sidebar::refresh_info_panel,
                        queue_bar::rebuild_queue_bar,
                        control_groups::refresh_group_bar,
                        perf_overlay::refresh_performance_overlay,
                        queue_bar::animate_queue_progress,
                        (
                            sidebar::announce_mission_messages,
                            sidebar::refresh_notifications,
                        )
                            .chain(),
                        (sidebar::refresh_objective, sidebar::refresh_match_time),
                        (
                            terrain_chunks::stream_terrain,
                            scenery_view::stream_scenery,
                            fog_view::shroud_unexplored_scenery,
                            terrain_chunks::finish_terrain_chunks,
                            terrain_material::dress_terrain_surfaces,
                        )
                            .chain(),
                        fog_view::paint_fog_texture,
                        fog_view::reveal_explored_scenery,
                        fog_view::show_seen_enemies,
                        fog_view::show_seen_projectiles,
                        minimap::redraw_minimap,
                        sidebar::show_outcome,
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
        app.add_systems(
            Last,
            match_flow::scope_match_entities::<Node>.run_if(
                in_state(match_flow::GameState::Playing)
                    .or_else(in_state(match_flow::GameState::Replay))
                    .or_else(in_state(match_flow::GameState::MapEditor)),
            ),
        )
        .init_resource::<campaigns::CampaignLibrary>()
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
                main_menu::set_up_side_preview,
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
                main_menu::run_side_preview,
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
        let settings = settings::load_settings();
        // Audio takes the global volume only when it starts, so music started before the first update
        // would keep playing at full volume, capture runs included.
        app.insert_resource(bevy::audio::GlobalVolume::new(sound::master_volume(
            &settings,
        )))
        .insert_resource(settings)
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
            fog_view::FogUploadPlugin,
            MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
            MaterialPlugin::<terrain_material::WaterMaterial>::default(),
            MaterialPlugin::<motion_material::MovingMaterial>::default(),
            MaterialPlugin::<crate::view::side_tint::SideTintedMaterial>::default(),
        ))
        .init_resource::<crate::view::side_tint::SideColors>()
        .init_resource::<crate::view::side_tint::SideTintedMaterials>()
        .add_systems(
            Update,
            (
                crate::view::side_tint::paint_other_players_sides,
                crate::view::side_tint::paint_new_parts,
                crate::view::side_tint::repaint_parts,
            )
                .chain(),
        )
        .add_systems(
            OnEnter(match_flow::GameState::Playing),
            crate::view::side_tint::paint_sides_for_a_new_battle,
        )
        .add_systems(
            OnEnter(match_flow::GameState::Replay),
            crate::view::side_tint::forget_chosen_colors,
        );
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
                terrain_chunks::spawn_terrain_visuals,
                camera::focus_on_player_base,
                sky::begin_weather,
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
                terrain_chunks::spawn_terrain_visuals,
                camera::focus_on_player_base,
                sky::begin_weather,
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
                sidebar::load_fonts,
                fog_view::create_fog_texture,
                sound::load_sounds,
                flat_scenery::load_flat_scenery_sources,
                crystal_models::load_crystal_models,
                battlefield::create_battlefield_assets,
                dressing::create_mine_look,
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
                catching_up::catch_up_with_the_battle
                    .run_if(in_state(match_flow::GameState::Playing)),
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
        terrain_chunks::stream_terrain,
        scenery_view::stream_scenery,
        terrain_chunks::finish_terrain_chunks,
        terrain_material::dress_terrain_surfaces,
        terrain_material::animate_water,
        battlefield::draw_rain,
    )
        .chain()
}
