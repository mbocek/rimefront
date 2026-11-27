# Ages

A battle is fought in one age. Every side builds that age's army and that age's defences; the other structures
are the same buildings in every age, only in its look. The game knows five ages, antiquity, gunpowder, the
industrial age, the modern age and the future, and has armies for three of them so far: antiquity, the age of
gunpowder and the modern age (the units of the campaign). The battle editor's *Age* setting picks one; missions are modern.

The plan the ages come from, a persistent world that goes through them towards a final race, is
[brainstorm-vekysveta-2026-11-26.md](../brainstorm-vekysveta-2026-11-26.md) (Czech).

## Lines

Each unit belongs to a line, its place in an army whatever the age (`UnitLine` in `crates/sim/src/sim/eras.rs`),
and each line has at most one unit per age. Defences have lines too (`DefenceLine`): tower, anti-aircraft gun,
bunker and wall.

| Line | Antiquity | Gunpowder | Modern |
| --- | --- | --- | --- |
| Infantry | Spearman | Musketeer | Rifleman |
| Ranged | Archer | Bombardier | Grenadier |
| Healer | Healer | Field Surgeon | Field Medic |
| Builder | Builder | Sapper | Engineer |
| Fast | Horseman | Dragoon | Scout Buggy |
| Main | Chariot | Cuirassier | Battle Tank |
| Siege | Catapult (Tech Center) | Field Gun (Tech Center) | Howitzer |
| Transport | Supply Wagon | - | Troop Carrier |
| Harvester | Ore Cart | Ore Wagon | Harvester |
| Tower | Watchtower | Cannon Tower | Gun Turret |
| Wall | Palisade | Palisade | Wall |

A line with no member in an age keeps what it has: a supply wagon stays a supply wagon through the age of
gunpowder and becomes a troop carrier in the modern one, and the palisade stands in both early ages
(`Eras::Span`).

The other modern lines, anti-armour, marksman, flame, raiders, heavy and stealth tanks, rocket artillery,
anti-aircraft, repair and aircraft, have no member before the modern age: an earlier army has none of them,
and nothing earlier becomes one. The Helipad stands only from the industrial age on, the Flak Cannon and the Rocket Bunker
only in the modern one. Both factions field the same army in the early ages.

In the age of gunpowder a line of muskets stops heavy cavalry, the cuirassier rides down bombardiers and field
guns, bombardiers scatter musketeers, and field guns shell any massed infantry. Cuirassiers charge at arm's
length like spearmen and horsemen.

Spearmen and horsemen fight at arm's length: they charge whatever comes within ten metres beyond their reach,
where gunmen look only six beyond theirs (`CHARGE_BEYOND_REACH` in `combat.rs`), so they do not stand under
arrows from just out of reach. Spears hurt horses more than men; the sabre the other way round.

A computer side does not split a wave that is mostly fighters hand to hand to send half of it round the flank:
foot soldiers on the long way arrive too late and both halves are beaten one after the other. With split waves
the Hard commander lost to Normal 7:14 in antiquity; with whole ones it wins 14:6. The numbers of the ancient
army are in [balance.md](balance.md#antiquity).

A mission's lists of allowed units and structures name lines: a level that allows the Rifleman and the Wall
allows the Spearman and the Palisade when it is ancient. A force's starting units and structures are deployed
as their line's member in the level's age, and left out when the line has none.

## When the age turns

The age is the level's (`LevelDefinition::era`); when it changes, `promotion::turn_the_age`:

- refunds, in full, everything queued for the old age and a finished structure of it waiting to be placed,
- forbids the old age's items and allows the new one's,
- spares every structure from damage for ten minutes (`CALM_AFTER_THE_AGE_TURNS_SECS`), so every side has
  time to promote; engineers can still capture,
- tells every side.

Structures that every age builds take on the new look at once. Old units and defences stay as they are, with
their old weapons, until promoted.

**Promotion.** *Promote into the new age* (key B) queues a promotion for each selected unit and defence that
has a successor. A promotion costs 40 % of the successor's price and takes half its build time
(`PROMOTION_PRICE_SHARE`, `PROMOTION_TIME_SHARE` in `production.rs`). A unit is promoted in the production lane
of its successor, so it takes the place of a new unit there, and only when it stands within 12 cells of a
structure that builds the successor when the order is given; a defence is rebuilt where it stands, in the
structures lane. The unit keeps its identity, place, orders and share of health, and its rank: its experience
is scaled by the two prices, so it is as far from the next rank as before, and a veteran's extra health carries
over. A promotion whose unit or defence has fallen or changed hands is dropped and refunded when it reaches the
front of its lane, and so is a defence that is being sold when its rebuilding is done.
A save or a saved world refunds queued promotions when it is loaded, because the simulation
ids they name are handed out afresh then.

When the age turns, `redress_promoted` (`src/view/dressing.rs`) gives a promoted unit or defence the model of
its new kind and links its turret, animations and wall runs again. `RIMEFRONT_CAPTURE_AGE=Modern` turns a
captured battle to that age two seconds in and promotes everything the player owns.

Computer sides promote too, every five seconds: defences first, then their dearest units near home, while they
keep 500 credits for new units and have at most two promotions waiting in a lane.

Nothing in a skirmish turns the age; a persistent world does, on a schedule.

## Ages of a world

A world goes through the ages on a schedule (`AgeSchedule` in `eras.rs`, `LevelDefinition::ages`): it starts
in the first age, and each age lasts at least `--era-days` and turns at the next `--era-hour` of the world's day
after that. A world's day is counted from its first tick, not by the server's clock, and stands still while the
server is down, so the hour drifts from the wall clock; the countdown in the game is what players go by. Only
ages with an army count, and the last of them lasts for good; the final age and how a world ends come with the
future's army. The schedule holds the age the world is in and the tick it began (`age`, `since`) and walks on
from there, so an age given an army in a later release is taken up at the next turn and never puts a world
back. It is worked out from the world's tick, never its `f32` clock, which stops advancing after twelve days of
world time; the seasons of a world are counted from the tick too now. A world grown by the server takes the
flags ([server.md](server.md)); a world the game runs alone has the defaults, a week an age from antiquity,
turning at the twentieth hour. A world saved before it had ages stays in the modern age without a schedule.

The world sends its schedule to every player as they enter (`Message::Entered`), and the game works the age out
from the world's tick as it does the season: the build menu, the models and the countdown below the objective
(the next age and the time left, in yellow in the last day) follow it, while the world alone turns the age and
refunds what was queued.

When the age turns:

- **A side that arrived late in the age** (in its last quarter, `LATE_IN_AN_AGE` in `world_host.rs`) has
  everything it owns at the turn promoted for free, wherever it stands, and is told how much.
- **The side of a player who is away** is promoted as far as its credits go, in the order computer sides use
  (defences, then the dearest units), wherever its units stand, without keeping anything back and never into
  debt. The player is told on return what was promoted and what was too dear (`SideNews::PromotedWithTheAge`);
  that tally is not saved, so after a restart they are not told.
- **The level's own side, while no player has taken it,** is promoted for free, so its first player starts in
  the age like any newcomer.
- Players present promote their sides themselves during the calm; computer sides as above. Nobody is told of
  a promotion that promoted nothing.

Measured with `rimefront-world-soak --days 1 --era-hours 2 --chance 4242` (20 players coming and going, Apple
M5 Pro, release build), the age turning at hour 20 promoted every side at once without a slow tick:

| World hour | Age | Tick, average | Tick, slowest | Units | Structures |
| --- | --- | --- | --- | --- | --- |
| 19-20 | antiquity | 2.31 ms | 22.4 ms | 284 | 280 |
| 20-21 (turns) | modern | 1.93 ms | 9.6 ms | 243 | 191 |
| 21-22 | modern | 1.44 ms | 20.3 ms | 236 | 185 |

The first nine hours of antiquity took 0.39-0.78 ms a tick, as modern worlds did; the slower hours after them
came with more sides and a wider world, not with the age. The soak writes the age into its last column.

A resumed world starts in the age of its saved tick without turning it again, with what was left of the calm
(the tick it turned on is saved), and the sides due a free promotion are saved too (layout 8, see
[compatibility.md](compatibility.md)).


## Looks

Units have their own models; scripted reinforcements arrive as their line's member in the level's age. A structure every age builds is shown as `ERA_LOOKS` in `src/view/assets.rs` says,
its modern model otherwise; when the age turns, `redress_buildings_for_the_age` swaps the models of the
structures standing. The ancient models are built by `tools/blender/rimefront_models/antiquity.py`, those of the age of gunpowder by
`gunpowder.py` beside it; their
sounds (`bow_shot`, `catapult_throw`, `blade_strike`, and `musket_shot` for the age of gunpowder) by
`tools/audio/sound_effects.py`, and the hooves
and creaking carts its horsemen, chariots and wagons make on the move (`loop_hooves`) by
`tools/audio/battle_noise.py`. Arrows and stones
fly as their own projectiles; a blow (`ProjectileStyle::Strike`) shows none.

## What changed for compatibility

The new unit kinds, structures, sounds, projectile styles and commands changed the world protocol to 9 (see
[compatibility.md](compatibility.md)). They were added at the end of their lists, so replays and saves of
modern battles read as before, and modern battles play exactly as before (`tests/fingerprints.txt` is
unchanged).
