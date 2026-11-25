# Balance

```sh
make balance              # 12 AI matches per pairing, about a minute on 10 cores
make balance MATCHES=40   # tighter numbers
cargo run --release --bin balance -- --matches 40 --personalities   # each commander personality against Balanced
```

`src/bin/balance.rs` runs the headless simulation (`rimefront::headless`) in parallel and prints a Markdown report:

- **Equal-cost duels:** $3000 of one unit type attack-moves into $3000 of another, on both sides of the river.
  The score is the row's remaining army value minus the column's (+100 = the row wins untouched).
- **AI against AI:** the computer plays the Coalition side too. With fog of war, Normal plays a mirror, then
  Hard and Easy challenge Normal and Brutal challenges Hard; a last mirror without fog shows what scouting changes. Sides alternate every
  match, so a favoured base site shows up in its own column.
- **Unit ledger:** how many of each unit the AIs built and lost, and the damage they dealt per credit spent.

Results after tuning (40 matches per pairing; the simulation is deterministic, so each pair of matches is played
on its own map variant, once from each corner):

| Duel ($3000 a side) | Rifleman | Rocketeer | Grenadier | Sniper | Scout Buggy | Raider Bike | Battle Tank | Stealth Tank | Bulwark | Howitzer | Gunship | Bomber | Fighter | Flak Track | Flame Trooper | Missile Truck |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Rifleman** | 0 | +83 | −85 | +42 | −79 | +76 | −50 | +63 | −16 | −91 | +80 | +48 | +100 | −16 | −10 | −90 |
| **Rocketeer** | −83 | 0 | −88 | −99 | −12 | +85 | +68 | +75 | +59 | −79 | +92 | +50 | +100 | +60 | −78 | −47 |
| **Grenadier** | +85 | +88 | −9 | −100 | −57 | +74 | −66 | −32 | −66 | −81 | −100 | −100 | 0 | −59 | +90 | −80 |
| **Sniper** | −42 | +99 | +100 | 0 | −98 | −95 | −97 | −96 | −97 | −98 | −100 | −100 | 0 | −96 | +23 | −97 |
| **Scout Buggy** | +79 | +12 | +57 | +98 | 0 | −10 | −49 | +32 | −58 | +26 | +83 | +16 | +100 | +80 | +89 | +32 |
| **Raider Bike** | −76 | −85 | −74 | +95 | +10 | 0 | −45 | +50 | −28 | +33 | +42 | −84 | +100 | +82 | −61 | +42 |
| **Battle Tank** | +50 | −68 | +66 | +97 | +49 | +45 | 0 | +64 | −12 | +62 | −100 | −100 | 0 | +87 | +79 | +68 |
| **Stealth Tank** | −63 | −75 | +32 | +96 | −32 | −50 | −64 | 0 | −67 | +48 | 0 | 0 | +100 | +75 | +21 | +41 |
| **Bulwark** | +16 | −59 | +66 | +97 | +57 | +28 | +12 | +67 | 0 | +70 | −100 | −100 | 0 | +86 | +81 | +70 |
| **Howitzer** | +91 | +79 | +81 | +98 | −26 | −33 | −62 | −48 | −70 | 0 | −100 | −100 | 0 | +11 | +88 | +14 |
| **Gunship** | −80 | −92 | +100 | +100 | −83 | −42 | +100 | 0 | +100 | +100 | 0 | +100 | −89 | −89 | +100 | +100 |
| **Bomber** | −48 | −50 | +100 | +100 | −16 | +84 | +100 | 0 | +100 | +100 | −100 | 0 | −100 | −50 | +100 | +100 |
| **Fighter** | −100 | −100 | 0 | 0 | −100 | −100 | 0 | −100 | 0 | 0 | +89 | +100 | 0 | −100 | 0 | 0 |
| **Flak Track** | +16 | −60 | +59 | +96 | −80 | −82 | −87 | −75 | −86 | −11 | +89 | +50 | +100 | 0 | +80 | −45 |
| **Flame Trooper** | +10 | +78 | −90 | −23 | −90 | +61 | −79 | −21 | −81 | −88 | −100 | −100 | 0 | −80 | 0 | −93 |
| **Missile Truck** | +90 | +47 | +80 | +97 | −32 | −42 | −68 | −41 | −70 | −14 | −100 | −100 | 0 | +45 | +93 | 0 |

| Challenger | vs | fog | wins : losses (unfinished) | median length | SW : NE site wins |
| --- | --- | --- | --- | --- | --- |
| Normal | Normal | on | 17 : 22 (1) | 7.7 min | 22 : 17 |
| Hard | Normal | on | 31 : 5 (4) | 7.9 min | 18 : 18 |
| Normal | Hard | on | 2 : 32 (6) | 7.5 min | 18 : 16 |
| Easy | Normal | on | 2 : 37 (1) | 8.3 min | 22 : 17 |
| Brutal | Hard | on | 36 : 3 (1) | 9.1 min | 23 : 16 |
| Normal | Normal | off | 14 : 25 (1) | 4.8 min | 27 : 12 |

The challenger plays the Coalition and the defender the Syndicate. The defender wins the Normal mirror more
often, but it does the same with both sides on one faction or with the factions swapped, so that is the
computer playing the player's team (which a person does in a real battle), not the factions. The south-west
corner also wins more often (about 3:2 in 300-match runs, most clearly without fog). It is not the map: every
open cell has a twin with the same ground and crystal, and both corners play the same opening move for move
until the commanders' dice first differ (`tests/symmetry.rs`). Scouting ties, ring scans, stacked-unit pushes
and the minefield direction were mirrored without changing it, so the cause lies later in the mid-game. A commander that picks a
vehicle it cannot afford yet saves for it instead of choosing again, so the heavy units of the Tech Center
(Bulwark, Missile Truck, Howitzer, Gunship) now appear in a long match. Every unit has a counter: riflemen beat rocketeers, flame troopers burn any infantry but melt before
vehicles, scouts hunt infantry, rocketeers kill armour, tanks and the Bulwark beat light vehicles, and howitzers
and missile trucks shell infantry but lose to anything fast that reaches them. Unfinished matches are mostly
mutual exhaustion: both sides out of units, money and harvesters, with only turrets left standing. Hard used to
think every 0.6 s and lost to Normal because of it; it thinks as often as Normal and wins on 30 % more income and
a first wave of eight instead of six. Brutal, the level above it, has 60 % more income, a first wave of nine and
waves that grow by three units instead of two. The new Grenadier beats infantry and loses to every vehicle, the
Raider Bike hunts artillery and loses to infantry, and the Stealth Tank loses a stand-up fight to the Battle Tank:
it is meant to fire first from hiding, which equal-cost duels do not show. The Fighter only shoots aircraft, so
against ground units its duels end even; the computer builds it only once it has seen enemy aircraft. Units now also prefer targets that can
hurt them (a tank picks the rocketeer before the rifleman next to it), which made matches shorter and left fewer
unfinished.

## Commander personalities

Each personality plays Normal against the Balanced commander on Normal, with fog of war, 40 matches per
pairing on alternating sides and maps (`--personalities`, a few seconds):

| Challenger | vs defender | fog | wins | losses | unfinished | median length | SW : NE site wins |
|---|---|---|---|---|---|---|---|
| Normal Rusher | Normal Balanced | on | 19 | 19 | 2 | 7.6 min | 26 : 12 |
| Normal Fortress | Normal Balanced | on | 23 | 11 | 6 | 6.0 min | 19 : 15 |
| Normal Armoured | Normal Balanced | on | 21 | 19 | 0 | 8.0 min | 27 : 13 |
| Normal AirPower | Normal Balanced | on | 16 | 24 | 0 | 6.6 min | 24 : 16 |

The Rusher and the Armoured commander are a match for the Balanced one; the Fortress wins about two games in
three, as its guns stop the early waves; the Air power commander loses three in five, because most matches are
decided before its Helipads pay off. Tuning that was measured and kept: the Armoured commander builds its first
Gun Turret right after the War Factory and no bigger first wave (it lost 6:12 before), and the Air power
commander builds its second Refinery before the Tech Center (14:26 before). A second early Gun Turret and no
penalty on tanks for the Air power commander did worse (12:27).

## Tried and rejected

Measured with the computer's change on one side only, against the unchanged commander on the other (40 matches per
pairing, Normal against Normal), so both sides of the result count:

- **Attack waves at one pace.** Holding a wave to its slowest unit won 31:7 and 31:9 without fog of war, but with
  fog, where nearly every game is played, it lost 11:26 and 16:19 (unchanged: about 15:22); a wave held to its
  median speed did worse (8:29). Waves still travel at each unit's own speed; only the player's group orders
  keep a pace.
- **Stealth Tanks kept in ambush.** Two Stealth Tanks lying still on the approach to the base changed nothing
  (22 against 23 wins for the side that builds them) and their damage per credit fell from 0.77 to 0.49, as they
  rarely met anyone.
- **Waves timed by the rain.** Setting off with three quarters of a wave while it rains, and holding a full wave
  for rain due within 45 s, changed no result (17:23 either way). After the first wave the computer sends each
  new unit to the front as it is built, so in rain it has 0-2 units waiting at home, and the first wave leaves
  before the first rain (4.5 minutes at the earliest on the test maps).
- **Gathering the army.** Once the enemy army has not been seen for 90 s the computer sends whatever waits at
  home, so most launches are trickles (839 against 649 full waves over a tournament, 515 of them a single unit).
  Waiting for half a wave (14:25 and 18:22 against about 17:23 unchanged) or for a full one (15:25, 18:22) made
  no difference, so trickling is not what decides these matches, and it is no foothold for the ideas above.
