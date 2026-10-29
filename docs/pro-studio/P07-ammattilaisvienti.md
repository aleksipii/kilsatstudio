# P07 — Ammattilaisvienti: OTIO, kuvasekvenssit, ProRes ja väriavaruus

Tila: **todo** · Aalto 1 · Haara: `feat/pro-p07-export` · Lähtö: `main` `d91994c`

## Tavoite

Kilsatin tuotos siirtyy suoraan ammattilaisten työkaluihin (Resolve, Premiere, Nuke, Avid):
- OpenTimelineIO-aikajana, jossa ovat otokset, leikkaukset ja dialogiraidat
- numeroitu PNG-sekvenssi, jonka nimeäminen ja metatiedot ovat hallittuja
- ProRes 422 HQ / 4444 (alfa) FFmpegin kautta
- selkeästi merkitty väriavaruus (sRGB / Rec.709)

Valinnaisena tulee EXR-sekvenssi. Se vaatii A01:n riippuvuuspäätöksen.

## Lähtötiedot

- `crates/motion-export/src/media.rs` (FFmpeg, nyt `libx264`), `sequence.rs` (`frame_%06d.png`),
  `streaming.rs`, `captions.rs`
- [media-export.md](../media-export.md), [frame-export.md](../frame-export.md),
  [scene-sequences.md](../scene-sequences.md)
- OTIO-skeema: avoin JSON-muoto (`.otio`). Kirjoitetaan omalla serde-koodilla, ei
  Python-riippuvuutta.

## Nykytila (koodista, tarkista)

- PNG-ruudut ja H.264+AAC-MP4 ovat olemassa. OTIO-, ProRes-, EXR- ja väriavaruustageja ei ole.

## Sallittu muutosalue

- `crates/motion-export`: uudet moduulit `otio.rs`, `prores.rs` ja (valinnainen) `exr.rs`,
  sekä testit
- `.github/workflows/export-qa.yml`: uudet vientitestit
- Uusi riippuvuus (esim. `exr`) vain A01-muotoisella päätöksellä ja PRO-orkestroijan
  hyväksynnällä

## Tehtävät — tarkistuslista

- [ ] OTIO-vienti: `Timeline` → `Stack` → video- ja audio-`Track`it → `Clip`it, joiden
      `RationalTime` lasketaan kanonisesta kellosta (24 kHz → fps, ei liukulukusummia).
      Viittaukset osoittavat vietyyn mediaan suhteellisilla poluilla.
- [ ] Skeematesti: kultainen tiedosto ja sen jäsennys takaisin. Valinnaisesti, jos
      `opentimelineio` on asennettuna, CI ajaa sen validoinnin. Merkitse puuttuvan työkalun
      tarkistus ohitetuksi, älä läpäistyksi.
- [ ] ProRes: `prores_ks`-profiilit `hq` ja `4444`. Alfa säilyy, kun tausta on läpinäkyvä.
      FFmpegin puuttuminen antaa selkeän virheen.
- [ ] Väriavaruus: sRGB/Rec.709-tagit (`-color_primaries`, `-color_trc`, `-colorspace`)
      ja PNG:hen `sRGB`/`gAMA`-palat. Dokumentoi, ettei ACES/OCIO-putkea ole.
- [ ] Peruutus ja edistyminen toimivat kuten nykyisessä viennissä (motion-jobs, rajattu jono).
- [ ] Päivitä [media-export.md](../media-export.md).

## Hyväksymisehdot

- OTIO:n kestot ja leikkauskohdat vastaavat MP4-viennin ruutuja tarkasti (testi).
- ProRes-vienti on ajettu oikeasti macOS:llä ja `ffprobe`-tulos kirjattu. Jos FFmpeg
  puuttuu, kirjaa este.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p07-export. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P07-ammattilaisvienti.md. Luo worktree haaralle feat/pro-p07-export,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
