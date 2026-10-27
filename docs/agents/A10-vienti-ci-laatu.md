# A10 — Vienti, CI ja laadunvarmistus

Tila: **todo** · Portit: G3–G4 · Tehtävät: KS-004, KS-005 · Lähtö: commit `496a8a4`

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

- [ ] Varmista ja dokumentoi, että vienti laskee jokaisen ruudun ajasta `t = frame / fps`
      samalla arvioijalla kuin esikatselu.
- [ ] Testi: ruutumäärä = `frame_count(kesto, fps)`; äänen kesto näytteinä vastaa videota
      sovitulla toleranssilla.
- [ ] Epäonnistuva vienti (puuttuva FFmpeg, täysi levy, peruutus) → tyypitetty virhe,
      osittaiset tiedostot siivotaan tai merkitään; testi.
- [ ] Toistettava vientikomento G2-näytekohtaukselle (README tai skripti).
- [ ] Regressio: G2-kohtauksen ruutuhashit tai kuvavertailu CI:hin.
- [ ] Mallittoman polun CI-ajo: ei mallilatauksia, ei synteesi-/tunnistinprosesseja.
- [ ] Suorituskykymittaus: laite, macOS, resoluutio, fps, kohtaus, commit; ei väitteitä
      60/120 FPS:stä tai 100 000 entiteetistä ilman mittausta.
- [ ] Kokoa G4-raportti: testit, vertailukuvat, macOS-käynnistys, vienti, mittaukset.

## Hyväksymisehdot

- Ruutumäärä, äänen kesto ja epäonnistuvan viennin käsittely on tarkistettu testein.
- CI ajaa uudet tarkistukset macOS:llä; tulokset linkitetty.
- Mittaustulokset sisältävät olosuhteet.

## Lopputulema

Odotettu: toistettava vientikomento, CI-laajennukset, regressiotarkistus, mittausraportti
ja G4-näyttökooste orkestroijalle.

### Toteutunut lopputulema

_Ei vielä aloitettu._

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
