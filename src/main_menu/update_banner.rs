use super::*;

#[derive(Component)]
pub struct UpdateBanner;

pub(super) fn update_banner_label(updates: &Updates) -> Option<String> {
    match &updates.state {
        UpdateState::NothingNew => None,
        UpdateState::Available(release) if updates.installs_itself() => Some(tr_fmt(
            "Version {} is out - update and restart",
            &[&release.version],
        )),
        UpdateState::Available(release) => Some(tr_fmt(
            "Version {} is out - download it",
            &[&release.version],
        )),
        UpdateState::Downloading { release, fraction } => Some(tr_fmt(
            "Downloading {}... {}%",
            &[&release.version, &((fraction * 100.0) as u32)],
        )),
        UpdateState::Installing(release) => Some(tr_fmt("Installing {}...", &[&release.version])),
        UpdateState::Installed(version) => Some(tr_fmt(
            "Version {} is installed - start the game again",
            &[version],
        )),
        UpdateState::Downloaded(path) => {
            Some(tr_fmt("Saved to {} - click to show it", &[&path.display()]))
        }
        UpdateState::Failed(reason) => Some(tr_fmt("Update failed: {}", &[&reason])),
    }
}

pub fn refresh_update_banner(
    updates: Res<Updates>,
    mut banners: Query<(&mut Node, &Children), With<UpdateBanner>>,
    mut labels: Query<&mut Text>,
) {
    let label = update_banner_label(&updates);
    let display = if label.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    for (mut node, children) in &mut banners {
        set_field(&mut node, |node| &mut node.display, display);
        if let Some(mut text) = children
            .first()
            .and_then(|&child| labels.get_mut(child).ok())
        {
            text.set_if_neq(Text(label.clone().unwrap_or_default()));
        }
    }
}
