# Kilsat Studio — PRO-agentit P01–P08 ja PRO-orkestroija

Luotu 29.10.2026. Lähtö: `main` `d91994c` (sama commit kuin `kilsatstudio1.0`).
Taustatutkimus: [ammattistudiotutkimus.md](ammattistudiotutkimus.md).

Tämä sarja tuo Kilsat Studioon ammattistudioiden käytäntöjä. Jokaisella agentilla on oma
tehtävätiedosto. Tiedostossa ovat tavoite, lähtötila, sallittu muutosalue, tarkistuslista,
hyväksymisehdot ja kopioitava tehtävänanto. Tiedostot ovat tehtävänantoja: niiden
olemassaolo ei tarkoita, että työ olisi tehty.

Sarja on erillinen arkkitehtuurisarjasta A01–A10 ([docs/agents](../agents/README.md)) ja
automaattiohjauksen sarjasta ([docs/automation](../automation/README.md)). Rajapinnat
sovitaan PRO-orkestroijan kautta. Mikään agentti ei luo kilpailevaa jäsennintä, kelloa
tai arvioijaa.

## Agentit

| ID | Tiedosto | Claude-agentti | Pääalue | Tila |
| --- | --- | --- | --- | --- |
| PRO-ORK | [Orkestroija](00-pro-orkestroija.md) | `kilsat-pro-orchestrator` | kokoaminen, PR:t, CI, `main`, `kilsatstudio1.0` | running |
| P01 | [Ajoitus ja blokkaus](P01-ajoitus-blokkaus.md) | `kilsat-p01-timing` | `motion-animation` (easing), `motion-scene` | done (#145) |
| P02 | [Suukartta ja X-sheet](P02-suukartta-xsheet.md) | `kilsat-p02-xsheet` | `motion-lipsync`, X-sheet-paneeli | done (#147) |
| P03 | [Animaatioperiaatteet](P03-animaatioperiaatteet.md) | `kilsat-p03-principles` | `motion-animation/src/rig_motion.rs`, `spring.rs` | done (#151) |
| P04 | [Elokuvakieli](P04-elokuvakieli.md) | `kilsat-p04-cinematography` | `motion-script/src/shots.rs`, validointi | done (#146) |
| P05 | [Taustat ja parallaksi](P05-taustat-parallaksi.md) | `kilsat-p05-backgrounds` | `motion-render`, `motion-library` (tasot) | done (#150) |
| P06 | [Esineet ja kiinnitys](P06-esineet-kiinnitys.md) | `kilsat-p06-props` | `motion-rig/src/props.rs`, `attachment.rs` | done (#144) |
| P07 | [Ammattilaisvienti](P07-ammattilaisvienti.md) | `kilsat-p07-export` | `motion-export` | done (#149) |
| P08 | [Model sheet ja tyylit](P08-model-sheet-tyylit.md) | `kilsat-p08-modelsheet` | `motion-render` (uusi moduuli), `motion-art` | done (#148) |

Tilat: `todo`, `ready`, `running`, `review`, `blocked`, `done`. Vain PRO-orkestroija
merkitsee tilan `done`-tilaksi, ja vain kun PR on yhdistetty ja CI on vihreä.

## Riippuvuudet ja suositeltu järjestys

```text
Aalto 1 (rinnakkain, eri tiedostot):  P01 · P02 · P04 · P06 · P07
Aalto 2 (tarvitsee aallon 1):         P03 (P01:n easingit) · P05 (P07:n kerrosvienti valinnainen)
Aalto 3:                              P08 (P05:n tasot, P06:n esinekortit)
Kokoaminen:                           PRO-ORK → PR → CI → main → kilsatstudio1.0
```

## Jaetut kuumat tiedostot

Näitä tiedostoja muuttaa useampi agentti. Vain yksi agentti saa kirjoittaa samaan
tiedostoon kerrallaan. Varaus merkitään alla olevaan taulukkoon PRO-orkestroijan
kautta, ja varattu PR yhdistetään ennen kuin seuraava aloittaa.

| Tiedosto | Tarvitsijat | Varaus |
| --- | --- | --- |
| `crates/motion-script/src/lib.rs` (MotionScript-avaimet) | P01, P03, P04, P06 | – |
| `crates/motion-script/src/story.rs` | P03, P04 | – |
| `apps/studio-desktop/src/main.rs` | P02, P05, P08 | – |
| `crates/motion-render/src/lib.rs` | P05, P08 | – |
| `docs/motionscript.md` | P01, P03, P04, P06 | – |

Kuumaan tiedostoon tehdään vain lyhyt kytkentä. Varsinainen toteutus menee uuteen
moduuliin, esimerkiksi `motion-script/src/timing.rs`.

## Yhteinen toimintaohje

1. **Lue ensin** juuren [AGENTS.md](../../AGENTS.md), tämä README, oma tehtävätiedostosi ja
   [ammattistudiotutkimus.md](ammattistudiotutkimus.md). Tarkista nykyinen koodi; älä
   oleta tehtävätiedoston nykytilakuvauksen olevan yhä ajan tasalla.
2. **Erillinen worktree.** Jokainen chat työskentelee omassa worktreessaan, jotta
   rinnakkaiset chatit eivät sotke toistensa tai käyttäjän keskeneräisiä muutoksia:
   ```sh
   git fetch origin
   git worktree add ../kilsat-pXX -b feat/pro-pXX-<aihe> origin/main
   ```
   Älä koske päätyöhakemiston keskeneräisiin muutoksiin (esim. haara `feat/story-studio`).
3. **Muutosalue.** Muuta vain omaa sallittua aluettasi. Uudet MotionScript-avaimet,
   yhteiset tyypit ja kuumat tiedostot sovitaan PRO-orkestroijan kanssa ennen muutosta.
   Vanhat `.kilsat`- ja YAML-tiedostot jatkavat toimimistaan (taaksepäin yhteensopiva).
4. **Ei uusia riippuvuuksia ilman päätöstä.** Kirjaa ehdotus lisenssin kanssa
   [a01-dependencies.md](../a01-dependencies.md)-muodossa ja pyydä PRO-orkestroijan hyväksyntä.
5. **Tarkistukset** worktreen juuresta:
   `cargo fmt --all --check`, `cargo check --workspace`,
   `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
   Kirjaa tarkasti, jos jokin tarkistus estyi.
6. **Commit ja PR.** Tee loogiset commitit, pushaa oma haarasi ja avaa PR `main`ia vasten
   (`gh pr create --base main`). Älä yhdistä itse, älä force-pushaa äläkä koske `main`iin
   tai `kilsatstudio1.0`:aan.
7. **Päivitä** oman tehtävätiedostosi tarkistuslista ja *Toteutunut lopputulema* vain
   tehdyn ja tarkistetun työn perusteella. Graafisen käyttöliittymän muutosta ei kuvata
   kokeilluksi, ellei sovellusta oikeasti ajettu.
8. **Palauta** PRO-orkestroijalle yhteenveto:

```markdown
### [P0X / tehtävä] — [päivämäärä]
- Tila: todo | ready | running | review | blocked | done
- Haara ja PR: feat/pro-p0X-…, URL
- Tehty: konkreettiset muutokset ja tiedostot.
- Tarkistettu: suoritetut komennot ja tulokset.
- Tekemättä: avoimet tehtävät ja täyttymättömät hyväksymisehdot.
- Estot ja riskit: syy ja ratkaisija.
- Seuraavaksi:
```

## Käynnistäminen eri chateissa

Avaa uusi Claude Code -chat tässä repossa ja kirjoita esimerkiksi:

```text
Toimi agenttina kilsat-p01-timing. Lue docs/pro-studio/P01-ajoitus-blokkaus.md ja tee sen tehtävät.
```

Agenttimäärittelyt ovat kansiossa [`.claude/agents/`](../../.claude/agents/) (`kilsat-p01-timing` …
`kilsat-p08-modelsheet`, `kilsat-pro-orchestrator`). Ne ohjaavat lukemaan tämän kansion
tehtävätiedostot, joten tehtävät ylläpidetään vain täällä. Kun kaikki PR:t ovat auki tai
yhdistetty, avaa chat agentille `kilsat-pro-orchestrator`.
