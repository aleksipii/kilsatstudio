# P08 — Model sheet, siluettitarkistus ja hahmotyylit

Tila: **todo** · Aalto 3 · Haara: `feat/pro-p08-modelsheet` · Lähtö: `main`

## Tavoite

Hahmo pysyy samannäköisenä koko tuotannon ajan:
- **Model sheet** tuotetaan suoraan rigistä: turnaround (edestä, puolisivulta, sivulta),
  ilmeet ja suumuodot, mittasuhteet päänkorkeuksina ja hahmon paletti.
- **Siluettitarkistus** näyttää hahmon täysin mustana ja varoittaa, jos asento ei erotu.
- **Tyyliesiasetukset** ovat omia, alkuperäisiä tyylisuuntia: tikku-ukko, rubber hose,
  litteä TV-tyyli ja geometrinen selitysvideotyyli.

## Lähtötiedot

- `crates/motion-art/src/style.rs`, `lighting.rs`
- `crates/motion-rig/src/definition.rs` (`MouthShapes`), `crates/motion-render`
- [stick-studio-art-direction.md](../stick-studio-art-direction.md), [art-format.md](../art-format.md),
  [rig-outlines.md](../rig-outlines.md), [custom-rigid-characters.md](../custom-rigid-characters.md)
- Tutkimus: [§4 Hahmotyylit](ammattistudiotutkimus.md#4-hahmotyylit). Älä kopioi
  esikuvateosten hahmoja tai grafiikkaa.

## Sallittu muutosalue

- Uusi moduuli `crates/motion-render/src/model_sheet.rs` ja `motion-art/src/style.rs`
  (esiasetukset)
- `apps/studio-desktop`: Model sheet -paneeli omaan moduuliin ja kytkentä `main.rs`:ään
  PRO-orkestroijan varauksella

## Riippuvuudet

- P02 (suukartan rivi), P05 (paletti- ja arvorakenne), P06 (esinekortin asettelu
  uudelleenkäyttöön). Voi aloittaa ilman niitä ja lisätä rivit myöhemmin.

## Tehtävät — tarkistuslista

- [ ] Model sheet: deterministinen PNG/SVG, jossa turnaround, 6 ilmettä, suumuodot,
      päänkorkeusviivain ja palettiruudut. Kaikki tulee rigin datasta.
- [ ] Siluettitarkistus: hahmon peitto mustana. Mittari kertoo, erottuvatko kädet
      vartalosta (pinta-alan tai ääriviivan pituuden muutos). Tuloksena varoitus, ei esto.
- [ ] Tyyliesiasetukset: viivan paksuus, ääriviiva päällä/pois, nivelten pyöristys,
      paletti ja `squash_stretch`-oletus (P03:n kenttä, jos yhdistetty). Valitaan
      `style: rubber_hose` jne.
- [ ] Testit: sama rigi tuottaa tavu tavulta saman kortin, ja jokainen esiasetus
      renderöityy (vertailukuvat).
- [ ] Dokumentoi tiedostoon `docs/model-sheet.md`.

## Hyväksymisehdot

- Nykyinen oletustyyli ei muutu.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p08-modelsheet. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P08-model-sheet-tyylit.md. Luo worktree haaralle feat/pro-p08-modelsheet,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
