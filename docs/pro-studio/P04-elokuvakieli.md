# P04 — Elokuvakieli ja leikkaussäännöt

Tila: **todo** · Aalto 1 · Haara: `feat/pro-p04-cinematography` · Lähtö: `main` `d91994c`

## Tavoite

MotionScriptin validointi tuntee elokuvakielen perussäännöt. Se varoittaa (ei estä),
kun käsikirjoitus rikkoo niitä:
- 180 asteen sääntö
- kuvakokojen hyppy (jump cut, sama kuvakoko peräkkäin samasta hahmosta)
- leikkaus kesken paikallaan olon, kun leikkaus liikkeen kohdalla olisi mahdollinen

## Lähtötiedot

- `crates/motion-script/src/shots.rs` (`ShotPreset`, `ShoulderSide`, `CutRules`, `expand_shots`)
- `crates/motion-script/src/auto_direction/director.rs`
- [shot-library.md](../shot-library.md), [camera-presets.md](../camera-presets.md),
  [camera-motion.md](../camera-motion.md)

## Nykytila (koodista, tarkista)

- Kuvakokoesiasetukset, olkakuvan puolet ja dialogiin sidotut leikkaussäännöt ovat olemassa.
- Säännönmukaisuuden varoituksia (180 astetta, jump cut) ei ole.

## Sallittu muutosalue

- Uusi moduuli `crates/motion-script/src/continuity.rs` ja sen testit
- `shots.rs`: vain varoitusten kytkentä. Kuuma tiedosto `lib.rs`: varaa PRO-orkestroijalta.
- Ei muutoksia `auto_direction`-ohjaajan päätöksiin. Varoitukset ovat tietoa, joita
  ohjaaja voi käyttää myöhemmin.

## Tehtävät — tarkistuslista

- [ ] Toimintalinja: kahden puhuvan hahmon välinen akseli lasketaan kohtauksen hahmojen
      sijainneista. Varoitus annetaan, kun kamera ylittää akselin peräkkäisten otosten
      välillä ilman välikuvaa (neutraali kuva tai kameran liike).
- [ ] Jump cut -varoitus: sama hahmo, sama kuvakoko ja pieni kulmamuutos peräkkäin.
- [ ] Leikkaus liikkeen kohdalla -vihje: ehdota leikkauskohtaa, jos lähellä on toiminnon
      alku. Vihje on tietoa, ei automaattinen muutos.
- [ ] Diagnostiikka erottelee tasot `warning` ja `hint` ja paikantaa varoituksen
      kohtaukseen ja otokseen. Varoitukset voi kuitata `continuity: allow: [axis_cross]`.
- [ ] Testit: akselin ylitys, sallittu ylitys kameran liikkeellä, jump cut ja kuittaus.

## Hyväksymisehdot

- Nykyiset esimerkit eivät tuota virheitä. Varoitukset on kirjattu ja tarkistettu järkeviksi.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p04-cinematography. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P04-elokuvakieli.md. Luo worktree haaralle feat/pro-p04-cinematography,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
