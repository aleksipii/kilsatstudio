# A08 — Renderöinti, kamera ja taustat

Tila: **ready** (palautus ORK-G2-02, kohta 1: puhelin kädessä G2-kohtaukseen, rajaustesti ja vertailukuvat; sopimus, syvyys ja rajaus yhdistetty PR #116 `9c69902`; tiedostot [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026)) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Kirjata ja täydentää renderöintisopimus: kohtausgraafi, syvyysjärjestys, laaja- ja
lähikuvat sekä yhtenäiset taustat tikku-ukkotyyliin; tuottaa vertailukuvat, joissa
sommittelu ja käteen kiinnitetty puhelin toimivat eri kuvakulmissa.

## Lähtötiedot

- `crates/motion-render/src/lib.rs`, `scene_commands.rs`, `gpu.rs`, `gpu_scene.rs`
- `crates/motion-art/src/lib.rs` (tasot, taso-järjestys, tausta/etualue), `lighting.rs`
- `crates/motion-scene/src/lib.rs` (`CameraPose`), `motion-script/src/camera_presets.rs`, `shots.rs`
- `apps/studio-desktop/src/camera_framing.rs`
- [composition.md](../composition.md), [camera-presets.md](../camera-presets.md),
  [shot-library.md](../shot-library.md), [gpu-parity.md](../gpu-parity.md),
  [stick-studio-art-direction.md](../stick-studio-art-direction.md)

## Nykytila (koodista)

- CPU/SVG-renderöinti ja valinnainen wgpu/Metal-polku; GPU-pariteettitestit `motion-export`issa.
- Kamera-asennot, esiasetukset ja otoslista ovat olemassa.
- Neljä taustaa ja 14 esinettä stick studio -paketissa (PR #98).
- Raportin `tween.wgsl` ja 100 000 entiteetin väite eivät ole nykyistä toteutusta.

## Sallittu muutosalue

Tarkka jako: [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026).

- `crates/motion-render/**`, `crates/motion-art/**` ja testit
- `docs/composition.md`, `docs/camera-presets.md`, `docs/art-rendering.md`, uusi `docs/render-contract.md`
- Vertailukuvat `docs/evidence/g2-render/`, testikohtaus `examples/g2-minimal-scene/`
- Ei `examples/stick-studio/**`, `motion-export`ia (A10) eikä `motion-scene`ä (pyynnöt A04:lle)
- Kameran rajausapu `camera_framing.rs` vain A09:n kanssa sovitusti.

## Riippuvuudet

- Edeltää: A04 (`EvaluatedScene`), A05 (tartuntapiste), A07 (suun tila).
- Kuluttajat: A09 (esikatselu), A10 (vienti, vertailukuvat).

## Tehtävät — tarkistuslista

- [x] Kirjaa renderöintisopimus: syöte `EvaluatedScene` → piirtokomennot; ei omaa kelloa.
      → `docs/render-contract.md`, `motion_render::contract`.
- [x] Syvyysjärjestys: tausta → hahmot → esineet → etuala; tasatilanteen järjestys
      dokumentoitu ja testattu. → `contract::paint_order`, `depth_violations` + testit.
- [x] Laaja- ja lähikuvan rajaussäännöt (esim. kasvot/käsi kuvan sisällä marginaalilla).
      → `motion_render::framing` (`WIDE` 4 %, `CLOSE_UP` 8 % + kasvot ≥ 20 %).
- [ ] Testikohtaus: kaksi tikku-ukkoa, tausta, puhelin kädessä, kameravaihto laajasta lähikuvaan.
      → Tehty **ilman puhelinta** (`examples/g2-minimal-scene/`); puhelin odottaa KS-003a:n `grip`ia.
- [ ] Vertailukuvat (PNG) kummastakin kuvakulmasta; tallenna hash ja renderöijä/alusta.
      → Tehty ilman puhelinta (`docs/evidence/g2-render/`); puhelinkuvat avoinna.
- [x] CPU/GPU-ero: kirjaa toleranssi; älä väitä pikselipariteettia ilman ajoa (ADR-A01-04).
      → `docs/render-contract.md` § CPU and GPU; G2-kohtausta ei ajettu GPU:lla.
- [x] Taustojen yhtenäinen tyyli: värit ja viivanpaksuus tikku-ukkotyylin ohjeen mukaan.
      → `motion_art::style::check_background`; neljä taustaa + G2-kohtaus läpäisevät.

## Hyväksymisehdot

- Puhelin pysyy kädessä ja kuvassa molemmissa kuvakulmissa (testi + vertailukuva).
- Syvyysjärjestys on deterministinen (testi).
- Nykyiset render- ja export-testit läpäisevät.

## Lopputulema

Odotettu: renderöintisopimus, rajaussäännöt, testikohtaus, vertailukuvat hasheineen.

### Toteutunut lopputulema

### KS-003c (A08) — 27.10.2026, haara `feat/a08-render-camera`
- Tila: review (puhelinosuus avoinna, odottaa KS-003a)
- Tehty: renderöintisopimus (`docs/render-contract.md`); `motion_render::contract`
  (`DepthBand`, `paint_order`, `depth_violations`); `motion_render::framing`
  (`Subject`, `FramingRule::{WIDE, CLOSE_UP}`, `check_framing`);
  `motion_art::style::check_background`; testikohtaus `examples/g2-minimal-scene/`
  (VEIKKO + KIPINA, pysäkkitausta, etualan tolppa, kova leikkaus 2,0 s laajasta
  lähikuvaan, generaattori toistaa tiedoston tavulleen); vertailukuvat ja hashit
  `docs/evidence/g2-render/`; viittaukset `docs/composition.md`, `docs/camera-presets.md`.
  Renderöijän tuloste ei muuttunut (vanhat golden-testit ennallaan).
- Tarkistettu: `cargo fmt --all --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets` (0 varoitusta) ja
  `cargo test --workspace --no-fail-fast`: 923 testiä läpi, 0 epäonnistui; yhteisen
  target-kansion kilpailutilanne kaatoi `motion-export`in doctest-käännöksen (E0463),
  joka läpäisi erikseen ajettuna (0 doctestiä). Ennen ajoa `cargo clean -p motion-render`
  ja `-p motion-art`. PNG:t tuotettu `motion-export`in `render_scene`-esimerkillä (resvg 0.45.1, CPU).
- Tekemättä: puhelin kädessä -testikohtaus, puhelimen rajaustesti ja -vertailukuvat
  (KS-003a `grip`); GPU-ajo G2-kohtauksesta; havainnollinen taidekatselmointi.
- Estot ja riskit: puhelin odottaa A05:tä (KS-003a). Hahmojen tasatilanne nojaa
  `motion-scene`n vakaaseen lajitteluun ID-järjestetyn kartan päällä (A04); testi
  vartioi sitä. `stick-studio.kilsat` sisältää vanhat leivotut `heldprop_*`-osat; G2
  käyttää hahmoarkkeja ilman niitä.
- Seuraavaksi: KS-003a:n yhdistämisen jälkeen A08 sitoo puhelimen `grip`illä
  VEIKKOn `hand_r`:ään, lisää sen `Subject`iksi molempiin kuvakulmiin ja päivittää kuvat.

## Kopioitava tehtävänanto

> Toimi agenttina A08. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A08-renderointi-kamera.md`. Säilytä tikku-ukkotyyli. Tee tarkistuslistan
> tehtävät, aja työtilan tarkistukset ja päivitä tarkistuslista sekä Toteutunut lopputulema.
