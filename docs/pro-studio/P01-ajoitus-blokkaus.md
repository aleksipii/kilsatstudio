# P01 — Ajoitus ja blokkaus

Tila: **todo** · Aalto 1 · Haara: `feat/pro-p01-timing` · Lähtö: `main` `d91994c`

## Tavoite

Animaattori pystyy tekemään pose-to-pose-työtä samalla tavalla kuin ammattistudioissa:
1. Ensin blokataan askeltavalla interpoloinnilla (stepped). Myöhemmin samat avaimet
   näytetään pehmeinä käyrinä.
2. Liikettä voi näyttää ykkösillä, kakkosilla tai kolmosilla, eli sama kuva pysyy 1, 2 tai
   3 ruudun ajan.

Molemmat ovat esikatselussa ja viennissä täsmälleen samat.

## Lähtötiedot

- `crates/motion-animation/src/lib.rs`: `Easing { Linear, SmoothStep, Hold }`, `Keyframe`, `Track`
- `crates/motion-scene/src/time.rs` (ruutu ↔ näyte; kanoninen kello 24 kHz,
  `motion_audio::clock`)
- `crates/motion-scene/tests/timeline_determinism.rs`
- [motionscript.md](../motionscript.md), [editable-timeline.md](../editable-timeline.md),
  [keyframe-editor.md](../keyframe-editor.md)
- Tutkimus: [§3 Animointitekniikat](ammattistudiotutkimus.md#3-animointitekniikat)

## Nykytila (koodista, tarkista)

- Askeltava pito on olemassa (`Easing::Hold`). Ease-in-, ease-out- ja back/overshoot-
  easingeja ei ole avainkohtaisina valintoina.
- Kakkosilla animointia (exposure) ei ole: arviointi tehdään jokaiselle ruudulle.

## Sallittu muutosalue

- `crates/motion-animation/src/lib.rs` (Easing) ja uusi moduuli `motion-animation/src/exposure.rs`
- `crates/motion-scene` (ajan kvantisointi ennen arviointia), sen testit
- MotionScript-avaimet: uusi moduuli `motion-script/src/timing.rs` ja lyhyt kytkentä
  tiedostoon `lib.rs`. Kuuma tiedosto: varaa se PRO-orkestroijalta.
- `docs/motionscript.md` (uusien avainten kuvaus), tämä tiedosto

## Riippuvuudet

- Ei edeltäjiä. Kuluttajat: P03 (easingit), P02 (X-sheet näyttää ruutuvalotuksen).

## Tehtävät — tarkistuslista

- [ ] Lisää easingit `EaseIn`, `EaseOut`, `EaseInOut` (kuutio) ja `Back { overshoot }`.
      Validoi overshoot: äärellinen ja rajattu. Vanhat projektit sarjoittuvat ennallaan.
- [ ] Tee exposure-kvantisointi: `on: 1 | 2 | 3` kohtaukselle ja hahmolle. Arvioitava
      aika pyöristetään alaspäin ryhmän ensimmäiseen ruutuun. Ääni ja suu eivät
      kvantisoidu, koska suu seuraa omaa kelloaan.
- [ ] Lisää blokkaustila `interpolation: stepped`, joka pakottaa kaikki avaimet
      Hold-tilaan arvioinnissa mutta ei muuta tallennettua dataa.
- [ ] MotionScript: `timing: { on: 2 }` kohtaukselle ja hahmolle sekä avainkohtainen
      `ease: ease_in_out | back(1.3)`. Tuntematon arvo antaa paikannetun virheen.
- [ ] Testit:
      - kakkosilla parittomat ruudut ovat identtiset edeltäjänsä kanssa
      - aikahyppy on sama kuin ruuduittainen toisto
      - easing-käyrät ovat monotonisia rajoillaan (Backia lukuun ottamatta)
      - tuntemattomat avaimet antavat virheen
- [ ] Päivitä `docs/motionscript.md` esimerkillä.

## Hyväksymisehdot

- Esikatselu ja vienti käyttävät samaa kvantisointifunktiota (testi).
- Vanhat esimerkit (`examples/**`) kääntyvät ja renderöityvät muuttumattomina.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p01-timing. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P01-ajoitus-blokkaus.md. Luo worktree haaralle feat/pro-p01-timing
origin/mainista, tee tarkistuslistan tehtävät, aja tarkistukset, avaa PR mainia vasten
ja palauta yhteenveto PRO-orkestroijalle.
```
