# P02 — Suukartta ja X-sheet

Tila: **review** (PR #147) · Aalto 1 · Haara: `feat/pro-p02-xsheet` · Lähtö: `main` `d91994c`

## Tavoite

Huulisynkkaan tulee studioiden vakiotyökalut:
1. **Klassinen suukartta:** Preston Blair / Rhubarb A–H + X. Kartta projisoidaan Kilsatin
   13 viseemistä, jotta piirretyt suumuodot voidaan vaihtaa ruuduittain.
2. **X-sheet:** ruutu ruudulta näkyvä taulukko, jossa ovat dialogi, foneemi, suumuoto ja
   avaimet. Sen voi viedä tekstinä tai CSV:nä.

## Lähtötiedot

- `crates/motion-lipsync/src/lib.rs` (13 viseemiä), `rhubarb.rs`, `manual.rs`,
  `audio_timeline` (`AudioTimeline::blend_at`)
- `crates/motion-rig/src/definition.rs` (`MouthShapes`)
- [audio-timeline.md](../audio-timeline.md), [manual-mouth-timing.md](../manual-mouth-timing.md),
  [rhubarb-estimated-cues.md](../rhubarb-estimated-cues.md), [viseme-quality.md](../viseme-quality.md)

## Nykytila (koodista, tarkista)

- 13 viseemiä on olemassa, ja jokaisen ajoituksen alkuperä (manual / estimated / forced)
  on tallessa. Rhubarb-vihjeiden tuonti on olemassa.
- A–H+X-projektiota ja X-sheet-näkymää tai -vientiä ei ole.

## Sallittu muutosalue

- `crates/motion-lipsync`: uudet moduulit `mouth_chart.rs` ja `xsheet.rs` sekä testit
- `apps/studio-desktop`: uusi paneelimoduuli `xsheet_panel.rs` ja lyhyt kytkentä `main.rs`:ään
  (kuuma tiedosto, varaa se PRO-orkestroijalta)
- Ei muutoksia 13 viseemin enumiin (A07:n sopimus).

## Riippuvuudet

- P01 (exposure-tieto X-sheetin sarakkeeseen; valinnainen, voi lisätä myöhemmin).

## Tehtävät — tarkistuslista

- [x] `MouthChart`-enum `A..H, X` ja kokonainen, dokumentoitu projektio 13 → 9. Testaa,
      että jokaisella viseemillä on täsmälleen yksi kohde.
- [x] `XSheet`: rivit ruuduittain kohtauksen fps:llä. Sarakkeet: ruutu, aikakoodi, puhuja,
      dialogin sana, viseemi, suukartta ja ajoituksen alkuperä. Ajat lasketaan kanonisesta
      näytekellosta, ei liukulukusekunneista.
- [x] Vienti: tasavälinen teksti ja CSV. Sama syöte tuottaa aina tavu tavulta saman tuloksen
      (testi).
- [x] Desktop-paneeli: X-sheet vierii toistopään mukana. Rivin klikkaus siirtää toistopään
      siihen. Raskas laskenta tehdään taustalla, ei tapahtumasilmukassa.
- [x] Dokumentoi kartta ja X-sheet tiedostoon `docs/xsheet.md`.

## Hyväksymisehdot

- Projektio ja X-sheet on testattu rajoilla: tyhjä ääni, kohtauksen raja ja offset/trim.
- Paneelia ei kuvata käsin kokeilluksi, ellei sovellusta oikeasti ajettu.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

30.10.2026, haara `feat/pro-p02-xsheet`, PR https://github.com/aleksipii/kilsatstudio/pull/147 (ei yhdistetty).

- `motion-lipsync/src/mouth_chart.rs`: A–H+X ja projektio 13 → 9. Rhubarb-tuonnin
  käänteinen. Kartoituksen kopio on poistettu tiedostosta `english/visemes.rs`.
- `motion-lipsync/src/xsheet.rs`: ruuduittainen X-sheet 24 kHz:n näytekellosta.
  Offset ja trim huomioidaan, virheet ovat tyypitettyjä, ja CSV- sekä tekstivienti
  ovat tavu tavulta deterministisiä.
- `studio-desktop/src/xsheet_panel.rs`: paneeli rakentaa X-sheetin taustasäikeessä.
  Se seuraa toistopäätä, ja rivin klikkaus siirtää toistopään. `main.rs`:ään tuli
  neljä riviä kytkentää ilman orkestroijan varausta, mikä on mainittu PR:ssä.
- `docs/xsheet.md`.
- Tarkistukset: fmt, check, clippy (ei varoituksia) ja test (1137 läpi, 0 epäonnistui,
  11 ohitettu) ajettiin worktreessa.
- Ei tehty: sovellusta ei käynnistetty (paneelia ei ole kokeiltu käsin). Sanakohtaista
  ajoitusta ei ole. Exposure-sarake odottaa P01:tä. Foneemisarake on paneelissa tyhjä.

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p02-xsheet. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P02-suukartta-xsheet.md. Luo worktree haaralle feat/pro-p02-xsheet
origin/mainista, tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
