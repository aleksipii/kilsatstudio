# A10 — Vienti, CI ja laadunvarmistus

Tila: **review** · Portit: G3–G4 · Tehtävät: KS-004, KS-005 · Lähtö: commit `496a8a4`

## Tavoite

Varmistaa ruutuaikaan perustuva vienti (`t = frame / fps`), äänen synkronointi,
regressiotarkistukset ja suorituskykymittaus; tuottaa toistettava vientikomento,
CI ja tulosraportti.

## Lähtötiedot

- `crates/motion-export/src/lib.rs` (`export_scene*`, `Progress`, `Report`, `Error`),
  `media.rs` (paikallinen FFmpeg), `sequence.rs`, `streaming.rs`, `tests/`
- `.github/workflows/*.yml` (audio, macos-preview, macos-release, stick-studio-preview, …)
- [frame-export.md](../frame-export.md), [media-export.md](../media-export.md),
  [fullhd-performance.md](../fullhd-performance.md), [practical-testing.md](../practical-testing.md),
  [build-evidence.md](../build-evidence.md), `docs/evidence/`

## Nykytila (koodista)

- PNG/SVG-ruutuvienti ja FFmpeg-koodaus käyttäjän valitsemalla paikallisella FFmpegillä.
- CI ajaa Rust- ja Python-testit macOS:llä ja Ubuntulla sekä esikatselu-/julkaisutyönkulut.
- Mallittoman ympäristön koko ketjun ajoa ei ole kirjattu.

## Sallittu muutosalue

- `crates/motion-export/**`, `.github/workflows/**`, `tests/`, `docs/evidence/`,
  vienti- ja suorituskykydokumentit.

## Riippuvuudet

- Edeltää: G2 (A04–A08), A09 (macOS-sovellus).
- Kuluttaja: orkestroija (G4-hyväksyntä).

## Tehtävät — tarkistuslista

- [x] Varmista ja dokumentoi, että vienti laskee jokaisen ruudun ajasta `t = frame / fps`
      samalla arvioijalla kuin esikatselu.
      → `encode_frame` käyttää `motion_audio::clock::frame_to_sample`a; testi
      `export_preview_parity_tests`; `docs/frame-export.md`.
- [x] Testi: ruutumäärä = `frame_count(kesto, fps)`; äänen kesto näytteinä vastaa videota
      sovitulla toleranssilla. → `tests/g2_export.rs` (G2 + 6 fps × 5 kestoa;
      toleranssi alle yhden ruudun).
- [x] Epäonnistuva vienti (puuttuva FFmpeg, täysi levy, peruutus) → tyypitetty virhe,
      osittaiset tiedostot siivotaan tai merkitään; testi. → `Error::EncoderUnavailable`,
      `Error::PartialOutput`; täysi levy oikealla 2 MiB RAM-levyllä.
- [x] Toistettava vientikomento G2-näytekohtaukselle (README tai skripti).
      → `--example g2_export`, [docs/evidence/g2-export](../evidence/g2-export/README.md).
- [x] Regressio: G2-kohtauksen ruutuhashit tai kuvavertailu CI:hin.
- [x] Mallittoman polun CI-ajo: ei mallilatauksia, ei synteesi-/tunnistinprosesseja.
- [x] Suorituskykymittaus: laite, macOS, resoluutio, fps, kohtaus, commit; ei väitteitä
      60/120 FPS:stä tai 100 000 entiteetistä ilman mittausta.
- [ ] Kokoa G4-raportti: testit, vertailukuvat, macOS-käynnistys, vienti, mittaukset.
      (Odottaa G3:a: A09:n käyttöliittymä ja puhelin G2-kohtauksessa, KS-003c.)

## Hyväksymisehdot

- Ruutumäärä, äänen kesto ja epäonnistuvan viennin käsittely on tarkistettu testein.
- CI ajaa uudet tarkistukset macOS:llä; tulokset linkitetty.
- Mittaustulokset sisältävät olosuhteet.

## Lopputulema

Odotettu: toistettava vientikomento, CI-laajennukset, regressiotarkistus, mittausraportti
ja G4-näyttökooste orkestroijalle.

### Toteutunut lopputulema

**A10 / KS-004 (vienti-QA) — 27.10.2026**, haara `feat/a10-export-qa`, pohja `main` `d9b88ae`.
Tila `review`; ORK päättää hyväksynnästä.

- **Kello ja arvioija.** `encode_frame` laskee näytteen kanonisella kellolla
  (`motion_audio::clock::frame_to_sample`, ORK-G1-04) eikä omalla kopiolla. Testi
  `export_preview_parity_tests` (G2, ruudut 0, 1, 23, 47, 48, 72, 95): aika on täsmälleen
  `i / fps`, viety SVG on sama kuin suora `evaluate` + `render_svg_with_art`, ja
  esikatselun (`ScenePreview::render`) pikselit ovat samat kuin viedyn SVG:n rasterointi
  esikatselukokoon.
- **Ruutumäärä ja äänen kesto.** `tests/g2_export.rs`: G2 = 96 ruutua = `frame_count(4, 24)`,
  manifestin jokaisen ruudun näyte, WAV 96 000 näytettä; lisäksi 6 ruudunopeutta
  (7, 12, 24, 25, 30, 60) × 5 kestoa (0,04–2,5 s, myös 1/3 ja 1,01 s). Sovittu toleranssi:
  ääni `round(kesto × 24000)` näytettä ja ero videon pituuteen alle yksi ruutu.
- **Epäonnistuva vienti.** Uudet virhetyypit `Error::EncoderUnavailable { path }` ja
  `Error::PartialOutput { path, cause, cleanup }` (alkuperäinen virhe säilyy, jäänyt
  hakemisto nimetään); siivous yhdessä `discard_partial`-funktiossa (`lib.rs`, `media.rs`,
  `sequence.rs`). Testit: puuttuva/suhteellinen FFmpeg, peruutus 10 ruudun jälkeen,
  I/O-virhe kesken sarjan, epäonnistuva siivous (unix, ei-root) ja oikea täysi levy
  (ENOSPC, 2 MiB RAM-levy; `#[ignore]`, CI macOS ajaa).
- **Toistettava komento.** `cargo run --locked --release -p motion-export --example g2_export
  -- OUT_DIR svg|png|mp4 [--ffmpeg PATH] [--runs N]`; JSON-yhteenveto. Ohje:
  [docs/evidence/g2-export/README.md](../evidence/g2-export/README.md).
- **Regressio.** Koko 96 ruudun SVG-sarjan tiiviste ja ruudut 24/72 kaikilla alustoilla;
  PNG-ruudut 24/72 Apple Siliconilla (samat kuin A08:n kirjaamat vientihashit).
- **Mallittoman polun CI.** Uusi työnkulku `.github/workflows/export-qa.yml` (macos-15):
  `ModelPolicy::Strict` + `prepare_scene_model_free`, ajo `sandbox-exec`issä (verkko ja
  prosessit kielletty, paitsi vientiohjelma ja MP4:lle FFmpeg), tyhjä `HOME`,
  `PATH=/nonexistent`; tarkistukset: `HOME` tyhjä, ei mallitiedostoja, hiekkalaatikon
  kontrolli estää `curl`in ja `/bin/echo`n. MP4: 96 dekoodattua ruutua, ääni/video < 1/24 s.
- **Mittaus.** Paikallinen: iMac21,2 Apple M1 8 GiB, macOS 27.0.1, rustc 1.99.1, release,
  G2 1920x1080 24 fps 96 ruutua: SVG mediaani 30,4 ms, PNG 1531,9 ms (7 ajoa + lämmittely),
  [local-apple-m1.json](../evidence/g2-export/local-apple-m1.json). CI-ajurin mittaus
  työnkulun yhteenvedossa. Kyse on offline-viennin ajasta, ei toisto- tai esikatselu-FPS:stä;
  60/120 FPS:ää tai 100 000 entiteettiä ei ole mitattu eikä väitetä.
- **Paikalliset tarkistukset** (macOS 27.0.1, Apple M1): `cargo fmt --all --check` ok;
  `cargo check --workspace` ok; `cargo clippy --workspace --all-targets` 0 varoitusta;
  `cargo test --workspace` 976 läpäisi, 0 epäonnistui, 9 ohitettu. Täyden levyn testi
  ajettu erikseen RAM-levyllä: läpi.
- **CI macOS** (`Export QA`, mittaus commitissa `c594cb8` [ajo](https://github.com/aleksipii/kilsatstudio/actions/runs/40558399213); head `d5203c0` [ajo](https://github.com/aleksipii/kilsatstudio/actions/runs/40560366530) läpi): GitHub `macos-15`,
  VirtualMac2,1 Apple M1 (virtual) 3 CPU 7 GiB, macOS 15.7.9 (24G830), rustc 1.99.1.
  `g2_export` 7 läpi / 1 ohitettu, pariteettitesti läpi, täysi levy RAM-levyllä läpi,
  hiekkalaatikon kontrolli estää verkon ja prosessit, mallittomat SVG/PNG 96 ruutua ja
  96 000 näytettä (SVG-sarjan tiiviste = kirjattu), MP4: 96 dekoodattua ruutua, video 4,000 s,
  ääni 4,000 s. Ajoaika (5 ajoa): SVG mediaani 31,9 ms, PNG 3038,0 ms. Ensimmäinen ajo
  (`5fa82c8`) kaatui skriptivirheeseen MP4-vaiheessa; korjattu `c594cb8`:ssä.
  Lisäksi `Audio foundation` (`rust` macOS + Ubuntu) ajaa uudet testit `--workspace`-ajossa.
- **Rajaus (ORK-katselmus).** Hiekkalaatikko todistaa, ettei verkkoyhteyksiä eikä
  aliprosesseja ole; mallittomuuden takaavat `ModelPolicy::Strict` ja `DisabledSpeechProvider`.
  G2:ssa ei ole puhetta, joten tuotua WAVia ei ole vielä ajettu tiukalla polulla. Striimaava
  MP4 (`streaming.rs`) ei vielä palauta `PartialOutput`ia.
- **Tekemättä.** G4-raportti (odottaa G3:a). MP4-vientiä ei ajettu paikallisesti (koneella ei
  FFmpegiä); se ajetaan CI:ssä. Fyysisen Macin GUI-esikatselun FPS:ää ei mitattu.
  KS-002h:n `tests/support`-kellokopio (`SAMPLE_RATE`-vakio) on yhä erillinen vakio.

## Tarkistukset

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

## Kopioitava tehtävänanto

> Toimi agenttina A10. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A10-vienti-ci-laatu.md`. Älä väitä testien tai mittausten onnistuneen
> ilman ajoa. Tee tarkistuslistan tehtävät ja päivitä tarkistuslista sekä
> Toteutunut lopputulema.
