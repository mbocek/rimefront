use std::collections::BTreeSet;

use bevy::prelude::*;

use crate::sim::catalog::{Diplomacy, Team};
use crate::sim::notifications::{News, SideNews};

/// Offers of alliance not answered yet, as (from, to). Two sides that offered each other become allies.
#[derive(Resource, Default, Debug)]
pub struct AllianceOffers(BTreeSet<(Team, Team)>);

impl AllianceOffers {
    /// Drops every offer made by or to a side that is gone, so its number starts clean when given out again.
    pub fn forget(&mut self, team: Team) {
        self.0.retain(|&(from, to)| from != team && to != team);
    }

    pub fn of(pairs: &[(Team, Team)]) -> Self {
        Self(pairs.iter().copied().collect())
    }

    pub fn pairs(&self) -> Vec<(Team, Team)> {
        self.0.iter().copied().collect()
    }
}

/// `from` offers `to` an alliance, or accepts the one `to` offered; `from` leaves any camp it was in.
pub fn offer_alliance(
    diplomacy: &mut Diplomacy,
    offers: &mut AllianceOffers,
    news: &mut MessageWriter<News>,
    from: Team,
    to: Team,
) {
    if from == to || !diplomacy.hostile(from, to) {
        return;
    }
    if !offers.0.remove(&(to, from)) {
        if offers.0.insert((from, to)) {
            news.write(News::to(to, SideNews::AllianceOffered(from)));
        }
        return;
    }
    offers.0.remove(&(from, to));
    leave_alliance(diplomacy, news, from);
    diplomacy.join_camp_of(from, to);
    news.write(News::to(from, SideNews::AllianceFormed(to)));
    news.write(News::to(to, SideNews::AllianceFormed(from)));
}

pub fn leave_alliance(diplomacy: &mut Diplomacy, news: &mut MessageWriter<News>, team: Team) {
    let former_allies = diplomacy.allies_of(team);
    if former_allies.is_empty() {
        return;
    }
    diplomacy.leave_camp(team);
    for ally in former_allies {
        news.write(News::to(ally, SideNews::AllianceLeft(team)));
        news.write(News::to(team, SideNews::AllianceLeft(ally)));
    }
}
