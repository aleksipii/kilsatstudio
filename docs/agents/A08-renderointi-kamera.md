# A08 — Renderöinti, kamera ja taustat

Tila: **ready** (G2, ORK-G1; renderöintisopimus `motion-render`, ORK-G1-06) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

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

- `crates/motion-render/src/**`, `crates/motion-art/src/**` ja testit
- `docs/composition.md`, `docs/camera-presets.md`, vertailukuvat `docs/evidence/`-kansioon
- Kameran rajausapu `camera_framing.rs` vain A09:n kanssa sovitusti.

## Riippuvuudet

- Edeltää: A04 (`EvaluatedScene`), A05 (tartuntapiste), A07 (suun tila).
- Kuluttajat: A09 (esikatselu), A10 (vienti, vertailukuvat).

## Tehtävät — tarkistuslista

- [ ] Kirjaa renderöintisopimus: syöte `EvaluatedScene` → piirtokomennot; ei omaa kelloa.
- [ ] Syvyysjärjestys: tausta → hahmot → esineet → etuala; tasatilanteen järjestys
      dokumentoitu ja testattu.
- [ ] Laaja- ja lähikuvan rajaussäännöt (esim. kasvot/käsi kuvan sisällä marginaalilla).
- [ ] Testikohtaus: kaksi tikku-ukkoa, tausta, puhelin kädessä, kameravaihto laajasta lähikuvaan.
- [ ] Vertailukuvat (PNG) kummastakin kuvakulmasta; tallenna hash ja renderöijä/alusta.
- [ ] CPU/GPU-ero: kirjaa toleranssi; älä väitä pikselipariteettia ilman ajoa (ADR-A01-04).
- [ ] Taustojen yhtenäinen tyyli: värit ja viivanpaksuus tikku-ukkotyylin ohjeen mukaan.

## Hyväksymisehdot

- Puhelin pysyy kädessä ja kuvassa molemmissa kuvakulmissa (testi + vertailukuva).
- Syvyysjärjestys on deterministinen (testi).
- Nykyiset render- ja export-testit läpäisevät.

## Lopputulema

Odotettu: renderöintisopimus, rajaussäännöt, testikohtaus, vertailukuvat hasheineen.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A08. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A08-renderointi-kamera.md`. Säilytä tikku-ukkotyyli. Tee tarkistuslistan
> tehtävät, aja työtilan tarkistukset ja päivitä tarkistuslista sekä Toteutunut lopputulema.
