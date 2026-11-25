use super::*;

#[derive(Component)]
pub struct JoinAddressText;

#[derive(Component)]
pub struct NetworkStatusText;

#[derive(Component)]
pub struct ChatDraftText;

pub(super) const ADDRESS_CHARACTERS: &str = "0123456789.:-abcdefghijklmnopqrstuvwxyz";
pub(super) const LONGEST_ADDRESS: usize = 64;

/// Typing on the network battle page: the host's address before connecting, a chat line afterwards.
pub fn type_on_network_page(
    menu: Option<Res<MainMenu>>,
    mut typed: MessageReader<KeyboardInput>,
    mut settings: ResMut<GameSettings>,
    mut lobby: ResMut<NetLobby>,
    mut address_fields: Query<&mut Text, (With<JoinAddressText>, Without<ChatDraftText>)>,
    mut draft_fields: Query<&mut Text, With<ChatDraftText>>,
) {
    let on_network_page = menu.is_some_and(|menu| menu.selection == Selection::Network);
    if !on_network_page {
        typed.clear();
        return;
    }
    let chatting = lobby.has_partner();
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        match (&key.logical_key, chatting) {
            (Key::Enter, true) => lobby.say(),
            (Key::Backspace, true) => {
                lobby.draft.pop();
            }
            (Key::Backspace, false) => {
                settings.join_address.pop();
            }
            (Key::Space, true) => lobby.draft.push(' '),
            (Key::Character(text), true) => lobby.draft.push_str(text),
            (Key::Character(text), false) => {
                for character in text.to_lowercase().chars() {
                    let fits = settings.join_address.len() < LONGEST_ADDRESS;
                    if ADDRESS_CHARACTERS.contains(character) && fits {
                        settings.join_address.push(character);
                    }
                }
            }
            _ => {}
        }
    }
    for mut field in &mut address_fields {
        field.set_if_neq(Text(settings.join_address.clone()));
    }
    for mut field in &mut draft_fields {
        field.set_if_neq(Text(format!("{}_", lobby.draft)));
    }
}

pub fn show_network_status(
    time: Res<Time<Real>>,
    lobby: Res<NetLobby>,
    mut lines: Query<&mut Text, With<NetworkStatusText>>,
) {
    let status = lobby.status(time.elapsed_secs()).unwrap_or_default();
    for mut line in &mut lines {
        line.set_if_neq(Text(status.clone()));
    }
}

pub(super) fn spawn_network_details(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    join_address: &str,
    lobby: &NetLobby,
    options: &SkirmishOptions,
) {
    panel.spawn(fonts.heading(tr("Network battle"), 30.0, ACCENT));
    panel.spawn(fonts.text(
        tr("Two to four players on their own computers. The host leads the faction chosen in the battle editor's settings and can still change them; the others take the computer corners. Anyone who joins once every seat is taken watches the whole battle. The host starts when everyone is ready. Everyone needs the same version of the game, and the host's port 47300 must be reachable."),
        15.0,
        TEXT_DIM,
    ));
    if !lobby.is_busy() {
        spawn_command_button(
            panel,
            fonts,
            tr("HOST A BATTLE"),
            MenuCommand::HostNetworkBattle,
        );
        spawn_address_row(panel, fonts, join_address);
        spawn_command_button(panel, fonts, tr("JOIN"), MenuCommand::JoinNetworkBattle);
        spawn_command_button(
            panel,
            fonts,
            tr("OPEN THIS ROOM ON THE SERVER"),
            MenuCommand::OpenServerRoom,
        );
        panel.spawn(fonts.text(
            tr("On a dedicated server, type ws://server/room/NAME above: the first to open the room picks the battle from the battle editor's settings, and everyone else joins the same address."),
            14.0,
            TEXT_DIM,
        ));
        spawn_command_button(
            panel,
            fonts,
            tr("ENTER THE SERVER'S WORLD"),
            MenuCommand::EnterServerWorld,
        );
        spawn_command_button(
            panel,
            fonts,
            tr("PLAY A WORLD ALONE"),
            MenuCommand::PlayWorldAlone,
        );
        panel.spawn(fonts.text(
            tr("A persistent world grows as players arrive; each newcomer gets a base of their own. Type ws://server/world above to enter a server's world, or play one alone. Early preview: crystal fields are not kept up to date yet."),
            14.0,
            TEXT_DIM,
        ));
    }
    panel.spawn((NetworkStatusText, fonts.text("", 16.0, GOOD)));
    if lobby.is_hosting() {
        let while_hosting: Vec<EditorField> = EDITOR_FIELDS
            .into_iter()
            .filter(|&field| field != EditorField::NetworkPlayers)
            .collect();
        spawn_editor_fields(panel, fonts, options, &while_hosting);
    } else if let Some(level) = lobby.offered_level() {
        spawn_level_facts(panel, fonts, level);
    }
    if lobby.has_partner() {
        spawn_chat(panel, fonts, lobby);
    }
    if lobby.can_accept_offer() {
        spawn_command_button(
            panel,
            fonts,
            tr("I'M READY"),
            MenuCommand::AcceptNetworkOffer,
        );
    }
    if lobby.can_start() {
        spawn_command_button(
            panel,
            fonts,
            tr("START THE BATTLE"),
            MenuCommand::StartNetworkBattle,
        );
    }
    if lobby.is_busy() {
        spawn_command_button(panel, fonts, tr("LEAVE"), MenuCommand::CancelNetworkBattle);
    }
}

pub(super) fn spawn_address_row(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    join_address: &str,
) {
    let label = tr("Host address (type it)");
    spawn_text_field(
        panel,
        fonts,
        label,
        join_address.to_string(),
        JoinAddressText,
    );
}

pub(super) fn spawn_chat(panel: &mut ChildSpawnerCommands, fonts: &UiFonts, lobby: &NetLobby) {
    for line in lobby.chat() {
        let who = if line.mine {
            tr("You").to_string()
        } else {
            tr_owned(&line.team.name())
        };
        let color = line.team.ui_color();
        panel.spawn(fonts.text(format!("{who}: {}", line.text), 15.0, color));
    }
    let label = tr("Message (Enter sends)");
    spawn_text_field(
        panel,
        fonts,
        label,
        format!("{}_", lobby.draft),
        ChatDraftText,
    );
}
