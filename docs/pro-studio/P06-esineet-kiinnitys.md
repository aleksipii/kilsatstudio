# P06 — Esineet, kiinnityspisteet ja esinekortit

Tila: **todo** · Aalto 1 · Haara: `feat/pro-p06-props` · Lähtö: `main` `d91994c`

## Tavoite

Nykyinen "puhelin kädessä" -ratkaisu yleistetään studion esinekäytännöksi:
- Esineellä on nimetyt kiinnityspisteet ja tilat (esim. auki/kiinni, päällä/pois).
- Esinettä voi siirtää kädestä toiseen ja laskea pöydälle kesken kohtauksen.
- Jokaisesta esineestä voi tuottaa esinekortin: mittakaava hahmoon nähden ja kaikki tilat.

## Lähtötiedot

- `crates/motion-rig/src/props.rs` (`HandSide`, `GripPlacement`, `HandPropItem`,
  `HandPropCatalog`), `attachment.rs`, `ik.rs`, `screen.rs`
- `crates/motion-library` (101 esinettä, ankkurit)
- [rig-attachment-ik.md](../rig-attachment-ik.md), [lighting-props.md](../lighting-props.md),
  [asset-library.md](../asset-library.md)

## Nykytila (koodista, tarkista)

- Käteen kiinnitys, tartuntasijoitus ja IK ovat olemassa. Esineen tiloja ja käden vaihtoa
  kesken kohtauksen ei ole yleisenä mallina.

## Sallittu muutosalue

- `crates/motion-rig/src/props.rs` ja uusi moduuli `prop_states.rs`
- `crates/motion-library` (manifestiin valinnaiset `states` ja `anchors`, taaksepäin
  yhteensopiva)
- MotionScript-tapahtumat `hand_off` ja `set_down`: uusi moduuli ja kuuman tiedoston
  `lib.rs` kytkentä PRO-orkestroijan varauksella

## Tehtävät — tarkistuslista

- [ ] Esineen tilat: nimetty joukko. Tapahtuma `prop_state` vaihtaa tilan ajassa, ja
      tuntematon tila antaa paikannetun virheen.
- [ ] Käden vaihto `hand_off: { from: left, to: right, at: 2.0 }` ja laskeminen
      `set_down: { anchor: table.top }`. Esineen maailmansijainti on jatkuva siirtymässä
      (testi: ei hyppäystä).
- [ ] Esinekortti: deterministinen SVG/PNG, jossa esine tikku-ukon mittakaavassa ja kaikki
      tilat ruudukossa. CLI-esimerkki ja testi kuvan mitoille.
- [ ] Validointi: viittaus olemattomaan ankkuriin tai esineeseen antaa tyypitetyn virheen
      ja toimintaohjeen.
- [ ] Päivitä [rig-attachment-ik.md](../rig-attachment-ik.md).

## Hyväksymisehdot

- Nykyiset puhelin- ja tablettiesimerkit (`examples/screen-demo/**`) toimivat muuttumattomina.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p06-props. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P06-esineet-kiinnitys.md. Luo worktree haaralle feat/pro-p06-props,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
