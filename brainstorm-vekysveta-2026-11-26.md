# Věky světa: dlouhodobé hraní v perzistentním světě

_Brainstorm — 2026-11-26_
_Jazyk: čeština · Režim: mapa variant, pak sbíhání k jednomu směru (varianta E)_

## Kontext

Perzistentní svět Rimefront už umí:
- růst;
- chránit základny nepřítomných hráčů a nechat je chátrat;
- spojenectví;
- střídat roční období jednou denně;
- dát hráči po návratu zprávu, co se stalo.

Chybí ale důvod zůstat ve světě týdny:
- Nic nekončí (`Objective::World` nikdy neskončí).
- Neexistuje skóre ani žebříček a statistiky se neukládají.
- Hráč nemá jméno, identita je jen náhodný `PlayerKey`.

Začínáme s kamarády, cílem je veřejnost.

Stav kódu, ze kterého rozhodnutí vycházejí:
- Katalog je napevno v `crates/sim/src/sim/catalog.rs`: 21 `UnitKind`, 15 `BuildingKind`, jeden statický `spec()` na druh. Na `.spec()` je v kódu 225 volání.
- Pořadí `UnitKind::ALL` ukládají záznamy her, nové druhy se proto přidávají na konec.
- `Level` už umí omezit dostupné jednotky (`available_units`, `TechLevel`).
- Období světa se počítají ze světového času (`Season::of_world_at(clock.elapsed_secs)` v `match_flow.rs`).
- Svět zakládá `rimefront-server --world SEED`.
- Veteránství existuje (`sim/veterancy.rs`).

## Předpoklady

Uživatel tyto body potvrdil slovem „pokračuj“, výslovně je nepotvrdil:
- **Pět ér:** Starověk → Střelný prach → Průmyslová → Moderní → Budoucnost. Dnešní jednotky tvoří éru Moderní.
- **První starší éra je starověk.**
- **Při přechodu éry se budovy mění zdarma, obrana se platí.**

Další předpoklady:
- Ekonomika je ve všech érách stejná: kredity, krystaly, sběrači, energie. Mění se jen vzhled a názvy.
- Šarvátka s kamarády nepotřebuje novou identitu, stačí `PlayerKey`.
- Modely a zvuky vznikají v generátorech (`tools/blender`, `tools/audio/generate_audio.py`). Ručně se neupravují.

## Hlavní směr

**Svět prochází érami od starověku do budoucnosti a končí finále, ve kterém se staví Projekt.**
- Éra je společná pro celý svět a mění se podle světového času. Hráči vidí odpočet.
- Nováček naskočí do éry, která právě běží.
- Jednotky se při přechodu povyšují za část ceny.
- Éry platí v celé hře: šarvátka si éru zvolí, kampaň zůstává v éře Moderní.
- Vítězové finále se zapíší do síně slávy a pak se založí nový svět.

### Klíčové technické rozhodnutí: éra je vlastnost druhu, ne nový rozměr statistik

Každá jednotka každé éry je **samostatný `UnitKind`** se statickým `UnitSpec`, jak je to dnes. `UnitSpec` dostane nová pole:
- `era: Era`;
- `line: UnitLine`, tedy role, ke které jednotka patří.

Funkce `UnitKind::in_era(line, era) -> Option<UnitKind>` a `UnitKind::upgraded_to(era)` hledají nástupce v linii. Povýšení **změní `UnitKind` existující entity** na nástupce. Entita si ponechá ID, pozici, podíl zdraví, veteránství i rozkazy.

Proč takhle:
- 225 volání `.spec()` se nemění.
- Dnešní jednotky dostanou `era: Modern` a simulace se nezmění.
- Nové druhy se přidávají na konec `ALL`, takže staré záznamy her zůstávají platné.

Dostupnost jednotek dnes řeší `Level::allows_unit` / `available_units`. Rozšíří se o podmínku `spec.era == current_era`.

**Budovy:**
- Výrobní a ekonomické budovy (velitelství, elektrárna, rafinerie, kasárna, továrna, Tech Center, heliport, opravna) **zůstávají jedním druhem pro všechny éry**. Pohled volí model podle éry `(BuildingKind, Era)` a název přes i18n. Statistiky se nemění, takže přechod je zdarma.
- Obranné stavby jsou **linie jako jednotky**. Je to `DefenceLine` (věž, protiletadlová, bunkr, zeď) se samostatnými `BuildingKind` pro jednotlivé éry, a povýšení mění druh budovy.

### Éry a role

| Linie (`UnitLine`) | Starověk | Střelný prach | Průmyslová | Moderní (dnes) | Budoucnost |
|---|---|---|---|---|---|
| Pěchota | Kopiník | Mušketýr | Puškař | Rifleman | Exovoják |
| Proti obrněným | — (kopiník bije jízdu) | Pikenýr | Protitankista | Rocketeer | Railgunner |
| Plošná / střelec | Lučník | Granátník | Kulometčík | Grenadier | Plazmometčík |
| Odstřelovač (Koalice) | — | — | — | Sniper | Pozdější rozhodnutí |
| Plamenometčík (Syndikát) | — | — | — | Flame Trooper | Pozdější rozhodnutí |
| Rychlá | Jezdec | Dragoun | Obrněný vůz | Scout Buggy | Vznášedlo |
| Hlavní úderná | Válečný vůz | Těžká jízda | Tank | Battle Tank | Mech |
| Obléhání | Katapult | Dělo | Houfnice | Howitzer | Railgunová baterie |
| Transport | Vůz | Povoz | Nákladák | Troop Carrier | Vznášecí transportér |
| Sběrač | Rudný vůz | Rudný vůz | Nákladní traktor | Harvester | Sběrný dron |
| Stavitel | Stavitel | Ženista | Ženista | Engineer | Technik |
| Léčitel | Ranhojič | Felčar | Zdravotník | Medic | Medbot |
| Vzduch | — | — | Dvojplošník (útočný) | Gunship, Fighter, Bomber | Dron, stíhač |
| Ostatní frakční (Bulwark, Missile Truck, Raider Bike, Stealth Tank, Repair Truck, Flak Track) | — | — | — | jako dnes | Pozdější rozhodnutí |

Odemykání:
- Frakční jednotky existují až od éry Moderní. Starší éry mají jeden soupis pro obě frakce.
- Vzduch a protiletadlová obrana se odemknou od Průmyslové éry. Heliport v dřívějších érách nejde postavit.
- Do starověku patří jen opravna a Tech Center (Tech Center odemyká katapult).

U koní a vozů rozhoduje `ArmorClass::Vehicle`, takže „kopí bije jízdu“ vychází ze stávající matice zbraní proti pancíři.

### Pravidla přechodu éry

- **Povýšení jednotky** stojí **40 % ceny nástupce** a trvá **50 % jeho doby stavby**. Hráč vybere jednotky a použije příkaz „Povýšit“. Jednotka musí být do **12 políček od vlastní výrobní budovy své linie**. Povýšení zabere výrobní frontu té budovy (kapacita je jedna jednotka zároveň na budovu). Hráč tak musí volit, jestli stavět, nebo povyšovat. Nedostatek energie zpomalí povýšení stejně jako výrobu.
- **Obranná stavba** se povyšuje na místě. Stojí 40 % ceny nástupce, trvá 50 % jeho doby stavby a v jedné základně se povyšuje vždy jen jedna stavba.
- Výrobní a ekonomické budovy se promění **okamžitě a zdarma**.
- **Rozpracovaná výroba** staré éry se v okamžiku přechodu zruší a vrátí v plné ceně.
- **Výzkumy** jsou týmové a platí ve všech érách beze změny.
- **Podpůrné schopnosti** mají stejnou mechaniku a jen nový vzhled podle éry. Průzkum provádí zvěd, balon a pak letoun. Palbu obstarají katapulty, děla a baterie.
- Jednotku ani obranu staré éry **nejde postavit**, jde jen povýšit. Staré jednotky bojují dál se svými statistikami.
- **Klid po přechodu**: 10 minut světového času od změny éry nedostávají budovy žádné poškození. Všichni tak mají čas povyšovat.
- **AI** (počítačoví sousedé i protivníci v šarvátce) vybírá jednotky **podle linie**: `VEHICLE_MIX` a další přepisuje přes `in_era`. Po přechodu povyšuje v pořadí obrana, pak jednotky od nejdražší a drží rezervu 500 kreditů. Nepovyšuje, dokud mu výroba nestojí kvůli nedostatku kreditů.

### Plán ér ve světě

- `rimefront-server` dostane volby `--era-days N` (výchozí 7), `--finale-days N` (výchozí 3), `--era-hour H` (výchozí 20) a `--first-era NAME` (výchozí Starověk, jakmile existuje).
- Plán je součástí uloženého světa.
- Přechod nastane v hodinu `H` světového dne, který následuje po uplynutí `N` dní od začátku éry. Počítá se ze světových sekund `MatchClock.elapsed_secs` stejně jako období, takže je deterministický a soak test ho zrychlí.
- Doporučené nastavení pro veřejný svět je `--era-days 28 --finale-days 7`. Je jen zdokumentované, výchozí hodnoty jsou nastavené pro kamarády.
- Klient dostane plán ve zprávě o světě a ukazuje **odpočet do další éry** v panelu stavu mise. V posledních 24 hodinách éry přidá upozornění.
- **Nováček** dostane základnu v aktuální éře. Když přijde v posledních **25 % éry**, jeho první povýšení po nejbližším přechodu je **zdarma**. Platí to pro všechny jednotky a obrany, které měl v okamžiku přechodu.
- **Nepřítomný hráč** povyšuje automaticky z vlastních kreditů ve stejném pořadí jako AI. Rezervu nedrží, ale do mínusu nejde. Zpráva po návratu vypíše, co se povýšilo a co ne.

### Šarvátka

- Nastavení šarvátky dostane volbu **Éra** (všechny existující éry, výchozí Moderní). `TechLevel` dál platí uvnitř éry: Basic bez těžkých linií, Standard bez frakčních těžkých jednotek.
- V šarvátce se éra nemění. Zrychlený průchod érami je mimo rozsah.
- Kampaň a mise zůstávají v éře Moderní.

### Finále a Projekt

- Poslední éra světa (Budoucnost) je **finále**. Trvá `--finale-days`.
- `BuildingKind::Project` jde postavit jen ve finále. Parametry:
  - cena 12 000;
  - stavba 600 s;
  - zdraví 20 000;
  - vlastní silná zbraň proti zemi i vzduchu;
  - spotřeba 200 energie.
- Najednou může stát víc Projektů.
- Na Projekt **se nevztahuje ochrana nepřítomných**, platí jen pro zbytek základny.
- **Vítězství**: Projekt přežije **12 hodin světového času** od dokončení. Zničení odpočet vynuluje.
- **Konec finále bez vítěze**: vyhraje strana, jejíž Projekt stál nejdéle. Když žádný Projekt nebyl dokončen, rozhoduje nejvyšší postup stavby. Pořadí 2. a 3. místa se určuje stejně.
- **Spojenectví**:
  - Strop je `max(2, ceil(počet aktivních lidských hráčů / 3))` členů. Aktivní hráč je ten, kdo byl ve světě za posledních 7 dní.
  - Na začátku finále se spojenectví **zamknou**: nové spojenectví nevzniká, vystoupit jde.
  - Kdo vystoupí, ztrácí podíl na vítězství spojenectví.
  - Projekt spojenectví vyhrává za všechny členy, kteří ve spojenectví zůstali.
- **Síň slávy**: soubor `hall_of_fame.json` ve `--world-dir` přežije konec světa. Záznam obsahuje:
  - seed světa;
  - datum;
  - pořadí 1.–3.;
  - pro každé místo stranu (barvu a frakci) a přezdívky členů.
- Po vítězství server uloží závěrečný stav do síně slávy a **založí nový svět** se seedem `seed + 1` v první éře.
- **Přezdívka**: krok 5 zavede volitelnou přezdívku. Zadává se v nastavení, posílá se při vstupu do světa a ukládá se k `PlayerKey`. Má nejvýš 16 znaků a server ji ořízne a vyčistí od řídicích znaků. Bez přezdívky se použije „Hráč N“. Neslouží jako účet a neověřuje se. Je to herní údaj, ne osobní údaj, proto se nesbírá nic dalšího.

## Pořadí stavby

Každý krok je samostatný commit (případně víc commitů, vždy po `make check`) a hra po něm zůstane hratelná.

1. **Model ér, bez nového obsahu.**
   - Přidá se `Era` (všech pět variant), `UnitLine`, `DefenceLine` a `UnitSpec.era`/`line`.
   - Vzniknou funkce `in_era` a `upgraded_to`.
   - `Level.era` (serde výchozí Moderní) se promítne do `allows_unit`/`allows_building`.
   - AI vybírá podle linie.
   - *Hotovo, když:* otisky v `tests/fingerprints.txt` zůstanou beze změny a testy pokrývají, že každá dnešní jednotka má linii a éru Moderní.
2. **Starověk a povyšování.**
   - Druhy: Kopiník, Lučník, Jezdec, Válečný vůz, Katapult, Vůz, Rudný vůz, Stavitel, Ranhojič.
   - Obrana: Strážní věž, Palisáda.
   - Nové `ProjectileStyle`: Arrow a Boulder.
   - Modely z `tools/blender`, včetně starověké podoby osmi budov. Zvuky (tětiva, kopyta, katapult) z generátoru.
   - Překlady pro cs a sk.
   - Volba éry v šarvátce.
   - Příkaz a pravidla povýšení, vrácení rozpracované výroby, klid po přechodu.
   - Přechod se dá spustit v simulaci. Test v `tests/` projde přechodem Starověk → Moderní.
   - Test v `tests/ai_tactics.rs` ověří, že AI povyšuje.
   - Test v `tests/playability.rs` ověří, že šarvátka ve starověku jde dohrát.
   - Dokumentace v `docs/eras.md`, s odkazy z `docs/balance.md` a README.
3. **Světy s érami.**
   - Volby serveru, plán v uloženém světě, odpočet v rozhraní.
   - Nováček naskočí do aktuální éry. Nepřítomný hráč povyšuje automaticky a zpráva po návratu to vypíše.
   - Zvýšení `WORLD_PROTOCOL` a `STORE_VERSION` s fixturami a čtečkou starého formátu. Starý svět se načte jako éra Moderní bez plánu.
   - Soak test `world_soak` se dvěma přechody, změřené časy ticku.
4. **Střelný prach, pak Průmyslová éra**, každá jako samostatný commit se stejnou sadou (druhy, modely, zvuky, překlady, testy). Průmyslová éra odemkne vzduch a protiletadlovou obranu.
5. **Budoucnost a finále.**
   - Jednotky Budoucnosti a Projekt.
   - Podmínka vítězství, zamykání a strop spojenectví.
   - Přezdívka, síň slávy a nový svět po vítězství.
6. **Veřejné světy.** Ochrana proti hraní za víc stran a moderace přezdívek. Až před otevřením veřejnosti, mimo tento plán.

Po kroku 2 se zahraje starověk proti moderní éře a rozhodne se, jestli pokračovat. Tím se ověří zábavnost celého směru.

## Zvažované varianty

- **A. Sezónní svět s resetem.** Zamítnuto, svět nemá čistý reset. Konec a nový začátek přežily uvnitř E.
- **B. Nekonečné pískoviště s osobním postupem.** Nerozvíjeno. E od něj převzal síň slávy.
- **C. Území a vliv.** Body vlivu prohrály s Projektem. Neutrální místa zůstávají zdrojem síly.
- **D. Válka dvou frakcí.** Nerozvíjeno, zůstává jako alternativa.
- **Roky bez resetu** (letopis každé 4 dny). Nahrazeno érami.
- **Spouštěč éry**: společný pokrok nebo závod o Div éry. Zamítnuto, vyhrál čas s odpočtem.
- **Povýšení zdarma.** Zamítnuto.
- **Éra pro každého hráče zvlášť.** Zamítnuto.
- **Finále zničením všech ostatních nebo body vlivu.** Zamítnuto.
- **Éry jen ve světech.** Zamítnuto, znamenalo by dvojí obsah.
- **Statistiky klíčované érou** (`spec(era)`). Zamítnuto, protože by změnilo 225 volání. Vyhrály samostatné druhy na éru.

## Rizika a jejich zmírnění

| Riziko | Pravděpodobnost / dopad | Zmírnění |
|---|---|---|
| Objem obsahu: zhruba 60–70 jednotek, hra mění identitu | vysoká / vysoký | Stavět po érách. Role se odemykají postupně. Modely z generátoru. Po kroku 2 se rozhodne, jestli pokračovat. |
| Hra uvízne napůl (moderní éra + starověk) | střední / vysoký | Krok 1 je užitečný sám o sobě. Starověk funguje i jako samostatná éra šarvátky. |
| Veteráni válcují nováčky | vysoká / střední | Nováček naskočí do aktuální éry, při pozdním příchodu má povýšení zdarma, začíná na okraji světa s ochranou. |
| Nepřítomný hráč se vrátí o éru pozadu | vysoká / střední | Automatické povyšování z kreditů, zpráva po návratu. |
| Bohatý hráč povýší všechno naráz | střední / střední | Jedna jednotka najednou na výrobní budovu, povýšení zabere výrobní frontu. |
| Přechod padne do noci | střední / střední | Volba `--era-hour`, 10 minut klidu po přechodu. |
| AI nepovyšuje | vysoká / střední | Pravidlo priorit a test v `ai_tactics.rs`. |
| Projekt drží ten, kdo nespí | střední / vysoký | Silná vlastní obrana Projektu, spojenectví napříč časovými pásmy. Pokud soak nebo hraní ukáže problém, přidají se okna bitvy. |
| Finále bez vítěze | nízká / střední | Rozhoduje nejdelší stání Projektu, jinak postup stavby. |
| Všichni se spojí | vysoká (veřejné) / vysoký | Strop spojenectví, zamknutí na začátku finále, pořadí 1.–3. |
| Více stran jednoho hráče | vysoká (veřejné) / vysoký | Krok 6 před otevřením veřejnosti. |
| Determinismus a kompatibilita | jistá / střední | V kroku 1 se otisky nesmí změnit. Nové druhy se přidávají na konec `ALL`. V kroku 3 se zvýší verze protokolu a úložiště a fixtury se znovu zaznamenají. Každá změna otisků se uvede v commitu. |
| Výkon: víc druhů a přechod mnoha jednotek najednou | nízká / střední | Přechod je jeden průchod přes entity. Soak test v kroku 3 změří časy ticku. |

## Otevřené otázky

Žádná z nich implementaci neblokuje. Jsou to ladicí hodnoty, které se mají změřit hraním nebo soakem, a výchozí hodnoty výše platí, dokud se nezmění:
- jestli 40 % / 50 % za povýšení drží rovnováhu;
- jestli je 12 hodin na udržení Projektu a 10 minut klidu správně;
- statistiky starověkých jednotek (vycházejí z dnešních hodnot linie, ladí se testem hratelnosti);
- jestli se mají přidat okna bitvy pro Projekt (jen když se objeví problém);
- frakční jednotky Budoucnosti (rozhodne se v kroku 5).

## Další kroky

- `/feature brainstorm-vekysveta-2026-11-26.md` pro kroky 1–3 v tomto pořadí. Každý krok jako samostatný commit po `make check`.
- Po kroku 2 zastavit kvůli hernímu testu starověku proti moderní éře.

## Vnější vstupy

- `scout`, soupis z kódu: 21 jednotek a 15 budov se statistikami napevno v `catalog.rs`. Ekonomika stojí na kreditech, krystalech a energii. Prerekvizity vedou přes Tech Center. Frakce mají odlišné soupisy. `UnitKind::ALL` se ukládá podle pořadí.
- `scout`, dřívější průzkum: `Objective::World` nikdy neskončí. Identita je `PlayerKey`. Neutrální místa, limit 120 jednotek a časování rozpadu základen.

## Výsledek

_Doplněno 2026-11-28._

**Směr vydržel.** Kroky 1–5 jsou hotové:

| Krok | Commit |
|---|---|
| 1 a 2 (model ér, starověk, povyšování) | `cafbd2a` |
| 3 (světy s érami) | `f706629`, `1d6c919` |
| 4 (Střelný prach) | `f68dd01` |
| 4 (Průmyslová) | `f500aaa` |
| 5 (Budoucnost a Projekt) | `f6d6aa8` |
| 5 (přezdívka) | `d30be56` |
| 5 (finále a síň slávy) | `00335d4`, `32e47a2` |

Krok 6 zůstává mimo plán. Ve všech krocích zůstaly bitvy moderní éry beze změny, takže se nezměnil `tests/fingerprints.txt`.

Rozhodnutí „éra je vlastnost druhu“ se osvědčilo. Každá éra přibyla bez zásahu do volání `.spec()` a bez změny otisků.

Co se oproti plánu změnilo:
- Kroky 1 a 2 vyšly jako jeden commit.
- Linie, která v nějaké éře nemá člena, si drží, co má. Například vůz zůstane vozem i ve Střelném prachu. Obrana může platit přes víc ér (`Eras::Span`): palisáda stojí v obou raných érách, zeď a protiletadlový kanón v Průmyslové i Moderní.
- Éry a období se počítají z ticku, ne z `MatchClock.elapsed_secs`. Hodiny v `f32` se po dvanácti světových dnech zastaví. Plán světa ukládá aktuální éru a tick jejího začátku, takže vydání, které doplní další éru, uložený svět nevrátí zpět.
- Strana, kterou zatím nikdo nedrží, se povyšuje zdarma.
- Projekt existuje jen v perzistentním světě, ne v šarvátce.
- Projekt se počítá, jen dokud jeho strana není pod ochranou nepřítomných (`32e47a2`). Plán Projekt z ochrany vyjímal. Šlo ale postavit Projekt, odhlásit se a vyhrát.
- Revize finále našla a uzavřela tyto mezery:
  - vyřazená strana odchází ze spojenectví;
  - strop spojenectví počítá skutečně přistupující stranu;
  - soubory chunků nesou seed, takže nový svět nepřepíše starý;
  - přezdívka nepropustí znaky nulové šířky ani znaky měnící směr textu.
- Frakční jednotky Budoucnosti nevznikly. Frakční linie (protitanková, odstřelovač, plamenomet, těžké a stealth tanky a další) mají člena jen v Moderní éře. Obě frakce mají v ostatních érách stejný soupis.
- Modely ostatních ér byly nejdřív hrubé zástupné tvary (500–1 600 trojúhelníků). V `f30061d` se předělaly na plné modely s opotřebením. Podrobnosti a měření jsou v `docs/development.md`.

Co zůstává otevřené:
- Ve hře s lidmi se nic neověřilo. Herní test starověku proti moderní éře, který měl proběhnout po kroku 2, není nikde zapsán. Ladicí hodnoty z „Otevřených otázek“ (40 % / 50 %, 12 h, 10 min klidu) tak dál čekají na hraní.
- Rovnováha (`docs/balance.md`):
  - Průmyslová éra má obtížnosti v nesprávném pořadí: Hard vyjde proti Normal nastejno a Brutal prohrává s Hard.
  - Ve Střelném prachu drží Easy krok s Normal.
- Rozdělit práci po érách a mít automatický nástroj rovnováhy (`balance --era`) bylo správné. Každá éra dostala měřenou matici protivníků dřív, než ji kdo hrál. Hraní ale nenahradí.
