# Missions and custom levels

The game opens on a start screen of large tiles: *Campaigns*, *Skirmish* (random battles and custom levels),
*Multiplayer*, *Editor* (battle editor and custom levels), *Replays* and *Settings* (graphics, sound, controls and
gameplay, the same pages as in the pause menu), with *Continue* above them when there is a quick save. Each tile
opens a screen with a *Back* button; Esc goes back too.
- **Campaigns:** the Coalition campaign of twelve missions and the Syndicate campaign of eight (S1-S8)
  (`assets/levels/*.json`), each mission with its own objective. A campaign is played in order: a mission unlocks
  once the one before it is won, and the start screen offers to *Continue campaign* with the next mission of the
  campaign played last. The campaign screen shows the route of missions (won, next and locked) beside the
  briefing of the chosen one: story, objective, map, sides and units. The campaign difficulty (Easy to Brutal)
  shifts every computer commander from its authored difficulty, one step down on Easy, up to two steps up on
  Brutal. After a campaign mission the end screen says how far the campaign has come and what victory unlocked. A level's `player_faction` decides which armoury the player builds from; every computer
  side fights for the other faction.
  1. Bridgehead (1 enemy, destroy everything; with a step-by-step tutorial)
  2. Hold the Ford (survive 8 minutes, introduces Rocketeers and Medics; in the rain)
  3. Iron Hammer (destroy both enemy Command Centers, introduces the Bulwark; on the lake)
  4. Three Fronts (three Hard enemies; on the plains)
  5. Silent Takeover (capture the walled-in enemy Command Center with an engineer; on the plains)
  6. Every Man for Himself (three cartels in a free-for-all; on the lake, in the rain)
  7. Rotor Storm (survive 12 minutes against a Crimson air force of gunships; flak and rocketeers; autumn)
  8. White Silence (destroy two Command Centers on a large winter plain in falling snow; snipers to scout)
  9. Marksmen (infantry only: snipers, medics and an engineer to capture the Crimson Command Center; autumn rain)
  10. The Iron Tide (two against two on a huge map: with Amber as an ally, destroy Crimson and Violet)
  11. Thunder Run (bombers and fighters over walled rocket bunkers; two bases to destroy, bomber reinforcements
      at 4 minutes and a Crimson counter-attack at 8; on the lake)
  12. Iron Lifeline (survive 14 minutes with Bulwarks and Repair Trucks: a Raider Bike raid at 3 minutes, a
      repair column at 6, stealth armour at 9; winter)
  - S1. Red Ledger (Syndicate: burn out a small Coalition post with Flame Troopers)
  - S2. Bunker Line (Syndicate: hold 12 minutes against Bulwarks and gunships behind Rocket Bunkers)
  - S3. Rocket Rain (Syndicate: shell the defences with Missile Trucks and destroy both Command Centers; large
    winter plain)
  - S4. Salted Earth (Syndicate: engineers mine the causeway over the lake; hold 10 minutes)
  - S5. Ghosts in the Pines (Syndicate: flush out camouflaged Coalition snipers along the river; autumn)
  - S6. Laboratory War (Syndicate: research all three upgrades against Bulwarks, then destroy both Command
    Centers; large plain in the rain)
  - S7. Hunting Season (Syndicate: Raider Bikes against the Coalition's harvesters; a bomber raid at 6 minutes,
    Stealth Tank reinforcements at 8; autumn plains)
  - S8. Ghost Armour (Syndicate: Stealth Tank ambushes and an engineer to capture the Coalition Command Center; a
    Bulwark column at 7 minutes, reinforcements at 10)
- **Choosing targets:** a unit firing on its own picks the enemy its weapon hurts most and that is closest to
  dying, preferring targets already in range and anything that shoots back over buildings, so a group
  concentrates its fire. Computer commanders send up to four soldiers idling at home to a harvester under fire.
- **Mines:** with engineers selected, M (rebindable) buries a mine under each of them for $100. A mine arms after
  1.5 s and goes off under the first enemy ground unit that touches it (110 damage with splash); its own side
  and allies see it as a small disc, enemies never do. Computer commanders with money to spare send an engineer
  to mine the approach to their base, up to four mines each. Save games keep the mines.
- **Camouflage:** a sniper that stands still for 3 s without firing is hidden from enemies: they neither see nor
  target it until one of them comes within 8 m. A shot gives it away for 2 s.
- **Main menu:** a battle scene from the game sits behind the menu; campaign missions won on this computer are
  marked *WON*, and a mission's description lists its objective, map, sides and units as a table.
- **Sidebar:** the objective and its progress (time left, credits gathered) sit under the credits, with the time played (match time, standing still while paused) beside the heading; selecting
  several units shows an icon and a count for each kind. Pointing at a build tile describes it (price, build time, what it needs and, for a unit, where it is built) in a box beside
  the sidebar, so the selection panel keeps showing the selection.
- **Pause menu and end screen:** the pause menu shows the mission, its objective, the time and the battle so far
  beside Resume; after a battle the end screen offers *Next mission* (after a campaign victory), *Play again*,
  the replay and the main menu.
- **Map preview:** the main menu shows the map of the selected mission, custom level, random battle or battle
  editor setup beside its description: ground, water, forests and rocks, crystal fields and every side's base,
  generated by the same code as the battle.
- **Random battle:** choose 1-3 computer opponents, their difficulty (or *Mixed*, a roll for each), the map size,
  the starting credits (the same for every side) and your faction (or a random one). The dice pick the landscape,
  season, weather, the corners, the alliances between several opponents and the shape of the land: how the
  river winds and where its outer fords lie, the lake's size, ponds on the plains, the hills and mountains. They
  also scatter extra forests, rock outcrops and crystal fields. Everything is point-symmetric around the map
  centre, so neither corner is favoured. A level's `terrain_seed` shapes the land; levels without one keep the
  classic landscape. Random battles, generated missions and battles from the battle editor (its *Neutral
  buildings* option, on by default) carry neutral sites and commanders of varied personalities (`"neutral_sites": true` and an enemy's
  `"personality": "Fortress"` in a level, see
  [rules.md](rules.md)); campaign missions and older levels have none. *New map* rolls
  again; changing the options keeps the map. The rolled battle can be played, saved as a custom level or opened
  in the map editor.
- **Battle editor:** 1-3 enemy commanders (allied or free-for-all), landscape, difficulty Easy/Normal/Hard/Brutal, starting credits, objective
  (destroy everything / destroy Command Centers / survive 10 minutes / stockpile credits), available units,
  map variant and your starting corner. *Save as custom level* writes a JSON file to
  `~/.config/iron_tide/levels/`; every valid file there is listed under *Custom levels*.
- **Quick battle:** the first page of *Skirmish*: pick a side (or random) and a difficulty, and one click starts
  a battle against one opponent on a standard map whose land, season, weather and corners are rolled anew.
- **Mission generator:** the second page of *Skirmish*: with the same side and difficulty choices it rolls a
  whole mission from one seed (`src/missions/mission_generator.rs`): a random battle given one of the campaign objectives
  (destroy everything, destroy the Command Centers, capture a Command Center, survive 8-14 minutes or stockpile
  $6,000-12,000), an English code name (*Operation Silent Anvil*) and a briefing written for the objective in the
  menu's language. *New mission* rolls another; it can also be saved as a custom level.
- **Custom campaigns:** a JSON file in `~/.config/iron_tide/levels/campaigns/` chains custom levels into a
  campaign listed beside the built-in ones, with the same unlocking, progress and difficulty:
  `{ "title": "Border Raids", "faction": "Coalition", "levels": ["a.json", "b.json"] }` (level file names in the
  custom levels folder, in order). *Add to my campaign* on a custom level appends it to `my_campaign.json`; a
  campaign whose level files are missing or invalid is left out.
- **Continue:** resumes the quick save. Saves remember their level.
- **Replays:** every match is recorded and saved when it ends (or as *Unfinished* when you leave it or close
  the game). The newest 10 are listed under *Replays* and kept in `~/.config/iron_tide/replays/`. Watching one
  rebuilds the map and shows every unit and structure where it was, walking and driving, with shells,
  missiles and explosions, without the fog of war: Space pauses, `+` / `-` changes the speed (1x-8x), the Left
  and Right arrows jump 10 s, Home starts over, Esc returns to the menu. The bar along the bottom shows how much
  has played and, as orange columns, where the shooting was; clicking or dragging on it jumps there. A replay
  stores snapshots twice a second plus the shots and blasts in between (a few MB for half an hour), not the
  inputs, so changed rules leave old replays intact ([determinism.md](determinism.md)).

A level file is plain JSON; omitted fields take defaults:

```json
{
  "name": "My battle",
  "briefing": "Text shown before the mission.",
  "map_seed": 1234,
  "player": { "site": "SouthWest", "credits": 5000, "buildings": ["PowerPlant"], "units": ["Rifleman", "Tank"] },
  "enemies": [
    { "site": "NorthEast", "credits": 5000, "difficulty": "Hard", "units": ["Rifleman", "Tank"] },
    { "site": "SouthEast", "credits": 4000, "difficulty": "Easy" }
  ],
  "objective": { "Survive": { "minutes": 10.0 } },
  "available_units": [],
  "available_buildings": [],
  "fog_of_war": true,
  "tutorial": false,
  "alliance": "CoalitionAgainstSyndicate",
  "landscape": "River",
  "weather": "Clear",
  "season": "Summer",
  "map_size": "Standard"
}
```

Landscapes: `River` (a winding river with three fords), `Lake` (a long lake between the bases with one
causeway across its middle) and `Plains` (open ground without water), all three point-symmetric, and `Wilds`:
rolling country with lakes, woods and crystal fields wherever the map seed puts them, generated chunk by chunk
(`world::wilds`, see [persistent-world.md](persistent-world.md)) and not mirrored. Every base in the wilds
stands on dry ground with a crystal field beside it; random battles roll only among the mirrored three.
Weather: `Clear` or `Rain` (falling rain, a dimmer sun and sky).
Map sizes: `Standard` (64 × 64 cells, every campaign mission), `Large` (96 × 96), `Huge` (128 × 128) and
`Vast` (256 × 256), also
in the battle editor. Landscapes, roads and crystal fields are authored for the standard map: on bigger maps
the bases stay in the corners, crystal fields and road bends near a base keep their distance from it, and
everything else spreads out from the centre (the fords move apart, the lake grows longer), while the river
keeps its width. Forests, rocks and meadows grow with the area, so a Huge map has about 2.5 times as much
scenery and costs more to draw. The map size is chosen when a match starts and kept per thread: every schedule runs on the thread that updates the app, so tests can play battles of different sizes side by side.
Seasons: `Summer`, `Autumn` (birches turn yellow, broadleaf trees orange, shrubs red, grass straw; pines stay
green; a warmer sun) or `Winter` (snow on open flat ground and on top of rocks, trees and logs, frosted
leaves, a colder sun, and rain falls as snow). Each scenery model's colours are turned once per season when
it is loaded, so seasons cost nothing while playing. The campaign's third and eighth missions are fought in winter, its fifth, seventh and
ninth in autumn.
Weather: `Clear` or `Rain` is how a match starts. From there the sky changes on its own: clear spells of a
few minutes, clouds that dim the sun and smooth away the moving cloud shadows, and showers that fade in,
soak the ground darker and glossier and dry off slowly afterwards. The forecast comes from the map seed and runs
on match time, so every player of a network match sees the same sky and a loaded save resumes it. Rain and snow
cut how far every unit and structure sees by up to a quarter (fog of war and what the computer knows; weapons
still fire as far as they reach), the player is told when it starts and stops, and *Rain* or *Snow* shows beside
the match time meanwhile.
Sites: `SouthWest`, `NorthEast`, `NorthWest`, `SouthEast`. Objectives: `"DestroyAllEnemies"`,
`"DestroyCommandCenters"`, `"CaptureCommandCenter"`, `{ "Survive": { "minutes": N } }`, `{ "ReachCredits": { "credits": N } }`.
`events` (`src/missions/level_events.rs`) script a mission: each happens once, when its trigger is met, and runs in the
simulation, so replays and network battles see it too and a save remembers which have happened.
Triggers: `{ "AfterMinutes": { "minutes": N } }`, `{ "StructuresLeft": { "team": "Crimson", "at_most": N } }`
(walls not counted) and `{ "UnitsLeft": { "team": "Crimson", "at_most": N } }`. The map editor's EVENTS page edits
them too. Actions: `{ "Message": { "text": "..." } }` (translated like a briefing),
`{ "Reinforcements": { "team": "Player", "units": ["Bomber"], "from": "NorthWest" } }` (they drive in from that
corner of the map; the player's head for its base, an opponent's attack-move on it) and
`{ "Credits": { "team": "Player", "amount": N } }`. A level whose events name a side not in the battle is refused.
Empty `available_*` lists allow everything. `"tutorial": true` shows the step-by-step tutorial panel
(select, move, power plant, refinery, barracks, a rifleman, attack-move, destroy the enemy); each step
completes as soon as the player does it, and the panel can be hidden. In every level a field tip explains
mines, research and camouflage the first time the player selects an engineer, owns a Tech Center or meets a
sniper; each shows once and is remembered in the settings. Difficulty (Easy, Normal, Hard, Brutal) changes how fast the AI
thinks, the size of its first attack wave (4 / 6 / 8 / 9), how fast its waves grow and its income (75 % / 100 % /
130 % / 160 %).
