# P04 — Elokuvakieli ja leikkaussäännöt

Tila: **review** · Aalto 1 · Haara: `feat/pro-p04-cinematography` · Lähtö: `main` `d91994c`

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

- [x] Toimintalinja: kahden puhuvan hahmon välinen akseli lasketaan kohtauksen hahmojen
      sijainneista. Varoitus annetaan, kun kamera ylittää akselin peräkkäisten otosten
      välillä ilman välikuvaa (neutraali kuva tai kameran liike).
- [x] Jump cut -varoitus: sama hahmo, sama kuvakoko ja pieni kulmamuutos peräkkäin.
- [x] Leikkaus liikkeen kohdalla -vihje: ehdota leikkauskohtaa, jos lähellä on toiminnon
      alku. Vihje on tietoa, ei automaattinen muutos.
- [x] Diagnostiikka erottelee tasot `warning` ja `hint` ja paikantaa varoituksen
      kohtaukseen ja otokseen. Varoitukset voi kuitata `continuity: allow: [axis_cross]`.
- [x] Testit: akselin ylitys, sallittu ylitys kameran liikkeellä, jump cut ja kuittaus.

## Hyväksymisehdot

- Nykyiset esimerkit eivät tuota virheitä. Varoitukset on kirjattu ja tarkistettu järkeviksi.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

PR [#146](https://github.com/aleksipii/kilsatstudio/pull/146), haara `feat/pro-p04-cinematography`, commit `e806ac3` (pohja `main` `c6ad82c`).

- Uusi moduuli `crates/motion-script/src/continuity.rs`. Tulokset palautetaan kentässä
  `ShotExpansion::continuity`. Tarkistukset:
  - `axis_cross` (warning): puhuvan parin järjestys ruudulla vaihtuu otosten välillä
    ilman neutraalia kuvaa.
  - `jump_cut` (warning): sama hahmo ja sama kuvakoko peräkkäin, ja rajaus muuttuu vain
    vähän (zoom alle 1,25×, siirtymä alle 10 % kuvan leveydestä).
  - `cut_on_action` (hint): leikkaus osuu paikallaan oloon, ja toiminto alkaa 0,5 s:n
    sisällä. Kenttä `suggested_cut` kertoo ehdotetun leikkauskohdan.
- Kuittaus otoskohtaisesti: `continuity: {allow: [...]}`. Kuitattu huomio jätetään pois.
- Ylitys, joka näkyy ruudulla saman otoksen aikana (hahmo kävelee toisen ohi), sallitaan.
  Neutraaleja kuvia ovat kuvat, joissa hahmot ovat linjassa, ja välikuvat, joissa kumpikaan
  hahmo ei näy. Olkakuvan puoli määrää parin järjestyksen.
- `shots.rs`: vain kytkentä. `lib.rs`: `mod continuity` ja uudelleenvienti (noin 5 riviä,
  orkestroijan vahvistettava). Dokumentaatio: `docs/shot-library.md`.
  `auto_direction` on koskematon.
- Tarkistettu (macOS, Apple Silicon): `fmt --check` ok, `check` ok, `clippy --all-targets`
  ok ilman varoituksia, `test --workspace` 1132 läpi, 0 epäonnistui, 11 ohitettu.
- Esimerkit: night-room-reference ei tuota huomioita (regressiotesti). Automaattiohjaajan
  tuotos ei tuottanut huomioita väliaikaisessa kokeilussa kolmella repliikkisarjalla.
- Tekemättä:
  - Yksittäisten kuvien katseen suuntaa ei päätellä.
  - Kynnysarvoja ei ole arvioitu oikeilla leikkauksilla.
  - Työpöytäsovellus ei vielä näytä huomioita.

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p04-cinematography. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P04-elokuvakieli.md. Luo worktree haaralle feat/pro-p04-cinematography,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
