# Rules

- **Factions:** Coalition (blue, the player) against up to three Syndicate commanders (Crimson red, Amber
  orange, Violet purple), each with its own base, economy and AI. They are allies and share what they see,
  unless the level says otherwise with `"alliance"` (or *Opponents* in the battle editor): `"FreeForAll"`
  makes everyone fight everyone, each seeing only through its own units and buildings, and `"TwoAgainstTwo"`
  pairs the Coalition with Amber against Crimson and Violet (the two western corners against the two eastern
  ones with the default sites); allies share what they see. Older levels' `"free_for_all": true` still works.
- **Economy:** the harvester mines crystals and unloads them at the refinery. Every refinery comes with a free harvester.
  Harvesters spread over a field, each to a crystal no other harvester has claimed, and drive through each other
  on their way so two of them never block a narrow lane. Once the fields near them are mined out they drive to
  the nearest crystal anywhere on the map. A crystal field is an irregular patch, richest in the
  middle, with a few poorer crystals scattered past its edge.
- **Power:** when consumption exceeds production, all production runs at half speed.
- **Engineer:** unarmed $500 infantry from the Barracks. Right-click an enemy structure (not a wall) with
  engineers selected: they walk in and capture it, damage and all. Right-click a damaged own structure: they
  repair it fully. Either way the engineer is used up. Other selected units attack the structure as usual.
- **Walls:** $50 concrete segments that block movement; segments of one side next to each other join into a
  continuous wall with a post on every cell, in play and in replays. Click the Wall tile, then drag on the map to lay a
  straight line (while dragging, the segments show green where they will stand and red where the ground or
  the credits stop them); each segment is paid as it is placed and the mode stays on until right click / Esc. Every
  segment extends the area you may build in, so walls can reach out from the base, but no structure or wall
  goes on ground its side has never explored; a wall dragged into the dark stops at its edge. Walls do not keep a
  defeated side alive, and the computer does not pick them as targets for its attack waves.
- **Neutral sites:** maps with neutral buildings have an Observation Post in the middle (or a pair of them
  beside it where the middle is a ford), two Supply Depots between the corners and two Field Hospitals on the
  roads from the corners to the middle, all mirrored like the rest of the map and kept clear of fords and
  passes. Nobody owns them and nothing can hurt them until an engineer captures one (right-click it with
  engineers selected); then a **Supply Depot** sends its owner $50 every 5 s, an **Observation Post** sees
  32 m around it, and a **Field Hospital** heals its owner's infantry within 9 m by 6 health a second. A
  captured site can be destroyed, which returns it to nobody, and cannot be sold; it does not keep a beaten
  side in the war. Computer commanders send an engineer to the nearest site their side has explored.
- **Repair and sell:** a structure under repair regains 4 % of its health per second and pays as it goes:
  a repair from rubble to full health costs half its price (the Command Center counts as $3000). Repairs pause
  while the credits run out and stop at full health; a green cross turns above it meanwhile. Selling refunds
  half the price and the structure sinks into the ground. The Command Center cannot be sold. The computer
  repairs its own structures below 60 % health when it has credits to spare.
- **Tech tree:** Command Center → Power Plant → Barracks / Refinery → War Factory → Tech Center, Repair Bay,
  Helipad; Barracks → Gun Turret, Flak Cannon, Rocket Bunker, Wall. The Bulwark, the Missile Truck, the Howitzer
  and the Gunship also need a Tech Center.
- **Factions:** the Coalition and the Syndicate share most of the arsenal, but each has its own: only the
  Coalition trains Snipers and builds Bulwarks, Repair Trucks and Bombers; only the Syndicate trains Flame Troopers (short-range flamers that
  burn infantry and buildings), builds Missile Trucks (wheeled rocket artillery, 18 m range), Stealth Tanks (hide like a sniper when they keep
  still and ambush armour with missiles; needs a Tech Center), Raider Bikes (fast, cheap rocket bikes that hunt
  harvesters and artillery) and Rocket Bunkers
  (a defense that hits vehicles and aircraft). The build menu shows only your own faction's items.
- **Units:** Rifleman, Rocketeer (anti-armour missiles), Grenadier (lobs grenades that blast bunched infantry and
  buildings, weak against vehicles), Sniper (long-range anti-infantry), Field Medic (heals
  nearby infantry), Engineer, Scout Buggy, Battle Tank, Bulwark (heavy twin-cannon tank), Howitzer (long-range
  artillery with splash damage), Flak Track (mobile anti-aircraft gun), Gunship, Harvester.
- **Map editor:** EDIT MAP on the battle editor page or on a custom level opens the map in the editor. Paint
  crystal fields, forests and rocks with the left button (keys 1-4 pick the brush), clear ground with the right
  one, in brushes of 1, 3 or 5 cells. Mirroring, on by default, paints the opposite cell too so both halves stay
  equally fair; base sites cannot be built over. SAVE writes the level (with its `map_edits`) to the custom
  levels, overwriting the custom level it was opened from. The LEVEL page sets the name, the briefing (click the
  field and type, Enter ends), the objective, and which corner each side starts in, with one to three computer
  opponents and their difficulty. Bases stay in the four corners, where the ground is levelled for them. The
  EVENTS page scripts the level one event at a time (`src/missions/event_editing.rs`): when (after minutes, or a side down
  to so many structures or units), an optional message typed like the briefing, and then reinforcements (an
  infantry squad, armour platoon or air wing of that side's faction, from a chosen corner) or credits; up to
  eight events, empty messages are dropped on saving. PLAY IT starts a battle on the level as it stands.
- **Research:** the RESEARCH tab of the build menu offers three one-time upgrades from the Tech Center, which
  must stand while the research runs: Composite Armour ($1500, vehicles and aircraft take 20 % less damage),
  High-Explosive Warheads ($2000, every weapon deals 15 % more damage) and Combat Drills ($1000, infantry
  reload 25 % faster). The computer researches them too once it has credits to spare.
- **Field repairs:** a vehicle standing still on a cell touching its own Repair Bay ($800, after the War Factory)
  regains 4 % of its health per second for free. The Coalition's Repair Truck ($600, War Factory, needs a Repair
  Bay) does the same in the field: it drives to damaged friendly vehicles within 12 m and repairs 12 health per
  second, for free.
- **Aircraft:** the Gunship (Helipad, $1100) takes off from the pad and flies straight to where it is sent, over
  water, walls and units, and tears through armour with its rocket pods. The Coalition's Bomber (Helipad and Tech
  Center, $1500) flies one bombing run, a single heavy stick of bombs with a wide blast, then flies back to the
  nearest own Helipad and takes no new target until it has rearmed (18 s); idle on its pad it lands. The Fighter (Helipad and Tech
  Center, $1000, both factions) hunts aircraft with air-to-air missiles, cannot hit anything on the ground and also
  lands on its pad when idle there. Bombers and Fighters take off and land straight up and down like a jump jet, folding their landing gear
  as they rise and lowering it to land, and only fly off once they are up at cruising height. Waiting in the air they
  circle where they stopped, banking into
  the turn, until they get an order or a target. Aircraft are selected and targeted by clicking them where they fly. An aircraft hovering over its own
  Helipad (within 3 m of the pad's centre, not moving) regains 4 % of its health per second for free. Tank cannons and howitzers cannot shoot upwards at all;
  riflemen and turrets hit it weakly, scout buggies better and rocketeers well. The dedicated counters are
  the Flak Track ($700, War Factory) and the Flak Cannon ($700 structure, after the Barracks), whose bursting
  shells do double damage to aircraft; the Flak Cannon cannot shoot at the ground at all.
- **Fair corners:** the map is point-symmetric around its centre: the river and its fords, the crystal fields
  and every tree or rock that blocks movement have a twin in the opposite corner. The last row and column, which
  would have no twin, are closed. Bases in the far half face
  the other way, so every base's doors, refinery dock and new units point towards the middle of the map.
- **Commander personalities:** a computer commander is *Balanced* (as before), a *Rusher* (attacks early and
  often with fast, cheap units and leaves its defences for later), a *Fortress* (rings its base with guns, waits
  longer and attacks with bigger waves behind artillery), *Armoured* (heads for the Tech Center, a Repair Bay
  and heavy tanks) or *Air power* (two Helipads as soon as it can, gunships and bombers). Random battles roll one
  for each commander, the battle editor sets one for all of them or mixes them (*Commander style*), and the
  level details show each commander's. Campaign missions keep the Balanced commander. Measured against each
  other in [balance.md](balance.md).
- **Support powers:** two buttons under the credits call help onto any spot of the map, then recharge. A
  **recon flight** (Z, needs the Command Center, every 2 minutes) reveals 16 m around the spot for 20 s, to
  your allies too. An **artillery barrage** (C, needs a Tech Center, every 3 minutes) drops ten shells within
  7 m of the spot after a 3 s warning, each hurting enemies within 3 m. Both start charging when the match
  begins. Computer commanders fly recon over a base site while they know no enemy structure and drop barrages
  on the thickest crowd of enemies they see, or else on the nearest enemy structure they know. Saves do not
  keep the charge: a loaded game starts it again.
- **Troop Carrier:** a $700 six-wheeled personnel carrier from the War Factory, for both factions, with a
  light machine gun. Right-click it with infantry selected and up to five soldiers climb in; U lets them out
  on free ground around it. Inside they ride along, cannot be hit or selected, and fire from the hatches at
  enemies within their reach (they hold position, or hold fire if told to). When the carrier is destroyed they
  scramble out with half their health. Loading a save puts passengers beside their carrier. The computer does
  not build carriers yet.
- **Stances:** an armed unit is *aggressive* by default: it takes on enemies it sees and chases them. A unit
  told to *hold position* (H) fires only at enemies within its weapon's reach and stays where it is; one told
  to *hold fire* (F) shoots only at what it is ordered to attack; G makes it aggressive again. Ordered attacks
  and moves work in every stance. The info panel shows the selection's stance, and saves keep it.
- **Veterancy:** units earn experience worth the price of every enemy unit or structure they destroy (the last
  hit counts). At their own price in experience they become Veterans (+20 % damage and health, one gold
  chevron), at three times it Elites (+40 %, two chevrons, and they slowly mend their own wounds). Structures
  that shoot do not gain ranks. Ranks are kept in saves.
- **Battlefield:** shells and explosions leave scorch marks that fade after 40 s; destroyed vehicles leave
  their charred hulls, smoking for a while, until they sink into the ground; damaged structures smoke and
  burn. All of it only where you can see.
- **Victory:** depends on the mission objective (destroy everything, destroy the Command Centers, capture one, survive,
  stockpile credits). The end screen compares you with your opponents (units trained and lost, structures built and
  lost, enemies destroyed, damage, credits harvested, match time) and offers the replay of the match.
- **Fog of war:** unexplored ground is black; explored ground stays visible but enemy units show only within
  the sight of your units and buildings, and so do shells, explosions and the smoke of damaged vehicles and
  structures. Enemy structures stay on the map once spotted. Your units only open
  fire on enemies someone on your side can see, so long-range artillery needs a spotter. The computer
  opponents play by the same rules: allied Syndicate commanders share one view of the map, remember the
  structures they have spotted, and send a scout (a Scout Buggy if they have one) and then their attack
  waves to base sites they have not explored yet. Once their economy stands they shield each gun turret
  with a short L of wall facing the middle of the map, and once they know an enemy structure they keep an
  engineer that follows their attack wave to capture it. They also count the enemy infantry, vehicles and
  aircraft they have seen lately (older sightings fade) and train more of whatever answers them best,
  judged by each unit's weapon against those armour classes; against aircraft they dig in up to two Flak
  Cannons first. Their vehicles below 35 % health fall back to a Repair Bay and their aircraft to a Helipad,
  and rejoin the fight at 90 %. Up to three scout buggies or gunships waiting at home dash off to hunt any
  enemy harvester they can see, and a wave of six or more splits: every other unit marches round a flank
  point 24 m beside the direct road before joining the attack. Levels can switch the fog off (`"fog_of_war": false`, or in the
  battle editor).
