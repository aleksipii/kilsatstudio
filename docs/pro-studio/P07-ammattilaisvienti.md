# P07 — Ammattilaisvienti: OTIO, kuvasekvenssit, ProRes ja väriavaruus

Tila: **review** (PR #149) · Aalto 1 · Haara: `feat/pro-p07-export` · Lähtö: `main` `d91994c`

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

Haara `feat/pro-p07-export` (worktree `.claude/worktrees/pro-p07`).

- OTIO: `otio.rs` – serde-kirjoitin ja -jäsennin, ajat kokonaislukuina 24 kHz kellosta
  ja vientien ruutusäännöstä; video-, miksaus- ja hahmokohtaiset dialogiraidat,
  ristihäivytys merkkinä (jo renderöity), suhteelliset polut. Kultainen tiedosto
  `tests/golden/two_scene_crossfade.otio` + takaisinjäsennys; ruututarkkuustesti 4/7/13/24/30 fps.
- ProRes: `prores.rs` – `prores_ks` HQ ja 4444 (alfa), oma kokoraja (oletus 2 GiB),
  peruutus/aikakatkaisu/osittaisen tiedoston siivous.
- Väri: `color.rs` – sRGB/Rec.709-tagit `setparams`-suotimella (FFmpeg 9 ohittaa pelkän
  `-color_trc`:n), Rec.709-matriisi; PNG:hen `sRGB`/`gAMA`. Ei ACES/OCIO-putkea.
- Oikea ProRes-ajo macOS:llä (FFmpeg 9.0.2), ffprobe: HQ `yuv422p10le` bt709/bt709/bt709,
  6 ruutua = vientimäärä + pcm_s16le 24 kHz; 4444 `yuva444p12le`, alfa 0/255 säilyy.
- Ei tehty: EXR (vaatii A01-päätöksen); opentimelineio-validointi paikallisesti (ei
  Python 3.15 -buildia) – CI ajaa sen ja merkitsee ohitetuksi, jos asennus ei onnistu;
  H.264-MP4:n väritagit (rajauksen ulkopuolella).

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p07-export. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P07-ammattilaisvienti.md. Luo worktree haaralle feat/pro-p07-export,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
