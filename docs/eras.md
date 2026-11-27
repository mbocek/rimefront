# Ages

A battle is fought in one age. Every side builds that age's army and that age's defences; the other structures
are the same buildings in every age, only in its look. The game knows five ages, antiquity, gunpowder, the
industrial age, the modern age and the future, and has armies for two of them so far: antiquity and the modern
age (the units of the campaign). The battle editor's *Age* setting picks one; missions are modern.

The plan the ages come from, a persistent world that goes through them towards a final race, is
[brainstorm-vekysveta-2026-11-26.md](../brainstorm-vekysveta-2026-11-26.md) (Czech).

## Lines

Each unit belongs to a line, its place in an army whatever the age (`UnitLine` in `crates/sim/src/sim/eras.rs`),
and each line has at most one unit per age. Defences have lines too (`DefenceLine`): tower, anti-aircraft gun,
bunker and wall.

| Line | Antiquity | Modern |
| --- | --- | --- |
| Infantry | Spearman | Rifleman |
| Ranged | Archer | Grenadier |
| Healer | Healer | Field Medic |
| Builder | Builder | Engineer |
| Fast | Horseman | Scout Buggy |
| Main | Chariot | Battle Tank |
| Siege | Catapult (Tech Center) | Howitzer |
| Transport | Supply Wagon | Troop Carrier |
| Harvester | Ore Cart | Harvester |
| Tower | Watchtower | Gun Turret |
| Wall | Palisade | Wall |

The other modern lines, anti-armour, marksman, flame, raiders, heavy and stealth tanks, rocket artillery,
anti-aircraft, repair and aircraft, have no ancient member: an ancient army has none of them, and nothing
ancient becomes one. The Helipad stands only from the industrial age on, the Flak Cannon and the Rocket Bunker
only in the modern one. Both factions field the same ancient army.

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

Nothing in a skirmish turns the age yet; persistent worlds will, on a schedule.

## Looks

Units have their own models; scripted reinforcements arrive as their line's member in the level's age. A structure every age builds is shown as `ERA_LOOKS` in `src/view/assets.rs` says,
its modern model otherwise; when the age turns, `redress_buildings_for_the_age` swaps the models of the
structures standing. The ancient models are built by `tools/blender/rimefront_models/antiquity.py`; their
sounds (`bow_shot`, `catapult_throw`, `blade_strike`) by `tools/audio/sound_effects.py`, and the hooves
and creaking carts its horsemen, chariots and wagons make on the move (`loop_hooves`) by
`tools/audio/battle_noise.py`. Arrows and stones
fly as their own projectiles; a blow (`ProjectileStyle::Strike`) shows none.

## What changed for compatibility

The new unit kinds, structures, sounds, projectile styles and commands changed the world protocol to 9 (see
[compatibility.md](compatibility.md)). They were added at the end of their lists, so replays and saves of
modern battles read as before, and modern battles play exactly as before (`tests/fingerprints.txt` is
unchanged).
