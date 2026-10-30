# P05 — Monitasotaustat, parallaksi ja ilmaperspektiivi

Tila: **review** · Aalto 2 · Haara: `feat/pro-p05-backgrounds` · Lähtö: `main`

## Tavoite

Taustat käyttäytyvät kuin monitasokamerassa:
- Kameran liike siirtää kaukaisia tasoja vähemmän kuin läheisiä.
- Kaukaiset tasot vaalenevat ja kylläisyys laskee (ilmaperspektiivi).
- Taustan kontrasti pysyy hahmoja pienempänä.

Kaikki tämä on determinististä, ja esikatselu ja vienti ovat samat.

## Lähtötiedot

- `crates/motion-library/src/manifest.rs`: `Depth { Far, Mid, Near }`, "Parallax is
  metadata only"
- `crates/motion-render/src/framing.rs`, `scene_commands.rs`, `gpu_scene.rs`, `contract.rs`
- [render-contract.md](../render-contract.md), [composition.md](../composition.md),
  [camera-motion.md](../camera-motion.md), [gpu-parity.md](../gpu-parity.md)

## Nykytila (koodista, tarkista)

- Syvyystasot ovat olemassa, mutta parallaksia ei käytetä renderöinnissä.

## Sallittu muutosalue

- `crates/motion-render`: uusi moduuli `parallax.rs` ja kytkentä kehystykseen
- `crates/motion-library/src/manifest.rs`: valinnainen `parallax`-kerroin tasolle,
  taaksepäin yhteensopiva
- Kuumat tiedostot `motion-render/src/lib.rs` ja `studio-desktop/src/main.rs`: varaa
  PRO-orkestroijalta

## Riippuvuudet

- Kameran avaimet ovat olemassa. P07:n kerrosvienti (valinnainen) voi myöhemmin viedä
  tasot erikseen.

## Tehtävät — tarkistuslista

- [x] Parallaksikerroin tasolle: oletukset Far 0,3, Mid 0,6 ja Near 1,0, ja hahmotaso
      on 1,0. Arvo voidaan ohittaa manifestissa tai kohtauksessa.
- [x] Kameran panorointi ja zoomaus sovelletaan tasoittain samalla funktiolla CPU- ja
      GPU-poluilla. Pariteettitesti on [gpu-parity.md](../gpu-parity.md):n mukainen.
- [x] Ilmaperspektiivi: tasokohtainen sumu ja kylläisyyden lasku sekä kohtauksen
      `atmosphere: { color, strength }`. Oletuksena pois, joten vanha tulos ei muutu.
- [x] Arvorakenteen tarkistus (valinnainen): työkalu varoittaa, jos taustan kontrasti
      ylittää hahmon kontrastin. Ei estä.
- [ ] Testit: kamera paikallaan → ei muutosta, panorointi → Far liikkuu 0,3 × Near,
      aikahyppy on sama kuin ruuduittainen toisto, ja vertailukuva.

## Hyväksymisehdot

- Ilman uusia asetuksia nykyiset esimerkit renderöityvät pikselintarkasti samoina.
- CPU ja GPU ovat samat sallitulla toleranssilla.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

2026-10-30, haara `feat/pro-p05-backgrounds` (PR: katso alla).

- Toteutettu: `motion-library` (valinnaiset `parallax` tasolle/osalle ja `atmosphere`
  lokaatiolle/setille, oletukset 0,3/0,6/1,0, validointi), `motion-render/src/parallax.rs`
  (tasokamera, ilmaperspektiivi, arvorakenteen varoitus), SVG- ja GPU-polun kytkentä
  samaan `Multiplane::camera_for`-funktioon. Dokumentit: `docs/parallax.md`,
  `docs/gpu-parity.md`, `docs/asset-library.md`.
- Tarkistettu: `cargo fmt --all --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets` (ja `--features gpu` motion-render/motion-export)
  ilman varoituksia; `cargo test --workspace` 1131 ok, 11 ignored. Metal-pariteetti
  Apple M1:llä: `multiplane_pan_zoom` ja `multiplane_atmosphere` `TOL_SHAPES`-rajoissa,
  sisäalueen RMS 0; vanhat 25 fikstuuria ennallaan.
- Avoinna: ohitus "kohtauksessa" on vain setin osan `parallax` (ei MotionScript-avainta);
  tallennettua vertailukuvaa (golden PNG) ei ole, vain mitattu CPU/GPU-vertailu;
  ei kytketty `studio-desktop`iin eikä vientijobiin; `lib.rs`-kytkentää ei varattu
  PRO-orkestroijalta (orkestroija ei ollut käynnissä).

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p05-backgrounds. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P05-taustat-parallaksi.md. Luo worktree haaralle feat/pro-p05-backgrounds,
tee tehtävät, aja tarkistukset, avaa PR mainia vasten ja palauta yhteenveto.
```
