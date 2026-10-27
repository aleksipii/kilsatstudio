# ORK — Orkestroija

Tila: **running** · Portit: G0–G4 · Lähtö: commit `496a8a4` · Viimeisin päätös: ORK-G1 (portti hyväksytty ehdoin), 27.10.2026 (`main` `393c2f1`)

## Tavoite

Ohjata agentit A01–A10 porttien G0–G4 läpi niin, että rajapinnat sovitaan ennen
toteutusta, rinnakkaiset työt eivät kirjoita samoihin tiedostoihin ja jokainen
`done`-merkintä perustuu todennettavaan näyttöön.

## Lähtötiedot

- [Arkkitehtuuriraportti](../KILSAT-STUDIO-ARKKITEHTUURI-JA-AGENTIT.md), luvut 6–10
- [Agenttien README](README.md) ja tehtävätiedostot A01–A10
- [A01-päätökset](../a01-architecture.md), [avoimet päätökset](../a01-open-decisions.md),
  [riippuvuudet](../a01-dependencies.md)
- [master-progress](../master-progress.md) ja [automaattiohjauksen suunnitelma](../automation/README.md)

## Sallittu muutosalue

- `docs/agents/**` (tilat, rekisteri, yhteenvedot)
- `docs/master-progress.md` (vain hyväksytyn yhdistämisen jälkeen, AGENTS.md:n mukaan)
- Ei tuotantokoodia; koodimuutokset tekee omistava agentti.

## Tehtävärekisteri

| ID | Omistaja | Riippuu | Tehtävä | Tila |
| --- | --- | --- | --- | --- |
| KS-001 | A01 + ORK | – | G0: koodikartoitus, riippuvuudet, No AI -politiikka, UI-linjaus | done (ORK-G0) |
| KS-002 | A02, A03, A04, A07 (+A05/A08) | KS-001 | G1: SceneSpec/TrackSpec/EventLog/AudioTimeline-sopimukset | done (ORK-G1: hyväksytty ehdoin KS-002f) |
| KS-002a | A02 | KS-001 | SceneSpec: versioitu kääntäjä, PR #105 (korvaa #102) | done (PR #105 `d9db5be`, ORK-G1) |
| KS-002b | A03 | KS-002a | EventLog v1 ja sääntösopimus | done ehdoin (PR #106 `6ddfdcc`, ORK-G1; ehto KS-002f) |
| KS-002c | A04 | KS-002a, KS-002b | TrackSpec-sopimus ja ajan muunnossäännöt | done (PR #107 `10135ad`, ORK-G1) |
| KS-002d | A07 | KS-001 | AudioTimeline v1 ja yhteinen näytekello | done (G1-osuus; PR #109 `ae39c4d`, ORK-G1-KS-002d) |
| KS-002e | A07 | KS-002d, KS-003 | Ihmisen äänittämät fi/en-WAV-esimerkit ja niitä vasten tarkistettu ajoitus; AudioTimelinen käyttö suun arvioinnissa | ready (G2) |
| KS-002f | A03 | KS-002b | EventLog: `sample_rate` sallii vain 24 000 Hz (`motion_audio::clock::DIALOGUE_SAMPLE_RATE`); muu taajuus tyypitetty virhe + virheellinen esimerkki | ready (estää EventLogin G2-kuluttajat A06/A09) |
| KS-002g | A04, A02, A10 | KS-002d | Kellokopiot (`motion_scene::time`, `auto_direction::SAMPLE_RATE`, `motion-export`in testituki) delegoimaan `motion_audio::clock`iin | ready (G2, ei estä) |
| KS-003 | A04–A08 | KS-002 | G2: pienin toimiva kohtaus | ready (ORK-G1) |
| KS-004 | A09, A10 | KS-003 | G3: käyttöliittymä ja koko ketju käsikirjoitus → video | todo |
| KS-005 | A10 + ORK | KS-004 | G4: CI, macOS-käynnistys, vienti, mittaukset | todo |
| KS-006 | A07 + A10 | KS-001 | `services/kokoro-runtime` Python-wheelien hash-lukitus (siirretty A01:ltä) | todo (G4-ehto) |
| KS-007 | A10 + ORK | KS-001 | Kolmannen osapuolen lisenssitekstit ja NOTICEt (siirretty A01:ltä) | todo (G4-ehto) |

## Tehtävät — tarkistuslista

### G0 — Nykytila
- [x] Lue luvut 6–10, kaikki agenttitiedostot ja nykyinen `master-progress.md`.
- [x] Vahvista A01:n katselmus (ADR-A01-01…05) tai palauta korjattavaksi. → ORK-G0: hyväksytty.
- [x] Päätä raportin ja nykykoodin erot: egui vs. SwiftUI (ADR-A01-01), 10 vs. 13 viseemiä,
      RETE-kirjasto vs. nykyinen `expand_rules`, `services/head-pose` (Apple Vision) No AI -luokitus.
- [x] Sovi päällekkäisyys [automaattiohjauksen agenttien](../automation/README.md) kanssa.

### G1 — Sopimukset
- [x] Määritä jokaiselle sopimukselle omistava crate: SceneSpec/TrackSpec (A02/A04),
      EventLog (A03), AudioTimeline (A07), rig-kiinnitys (A05), renderöintisopimus (A08).
      → ORK-G1-01…06.
- [x] Hyväksy yhteinen versio ja esimerkit; kirjaa ne `docs/agents/`-tiedostoihin. → ORK-G1 (portti).
- [x] Merkitse G2-tehtävät `ready`-tilaan vasta sopimusten hyväksynnän jälkeen. → A04–A08 `ready`.

### G2 — Pienin toimiva kohtaus
- [ ] Jaa tiedostot ilman päällekkäisiä kirjoittajia (A04 motion-scene, A05 motion-rig,
      A06 motion-animation-fysiikkamoduuli, A07 ääni-cratet, A08 motion-render/motion-art).
- [ ] Käynnistä rinnakkain vain riippumattomat työt; kirjaa jokaisen haara.
- [ ] Tarkista näyte: kaksi tikku-ukkoa, tausta, puhelin kädessä, kameravaihdos, WAV-puhe.

### G3 — Integrointi
- [ ] A09 liittää käyttöliittymän; A10 ajaa koko ketjun ja virhe-/peruutustilanteet.
- [ ] Palauta epäonnistunut työ omistajalle kirjatulla syyllä.

### G4 — Julkaisunäyttö
- [ ] Kokoa A10:n raportti: testit, vertailukuvat, macOS-käynnistys, vienti, mittaukset.
- [ ] Hyväksy vain osoitetut ominaisuudet; kirjaa rajaukset. KS-006 ja KS-007 ovat G4:n ehtoja.
- [ ] Päivitä raportin luvun 8 tarkistuslista näytön perusteella.

### Jatkuvat
- [ ] Rajapintamuutos avaa siitä riippuvat tarkistukset uudelleen.
- [ ] Jokaisesta luovutuksesta luvun 10.2 yhteenveto.
- [ ] Ei yhdistämistä `main`-haaraan ilman käyttäjän nimenomaista lupaa.

## ORK-G0 — Nykytilan portti: **hyväksytty** (27.10.2026)

**Näyttö.**

- PR #101 (`cdff71d`) ja PR #103 (`331f9d4`) yhdistetty `main`iin; PR #103:n tarkistukset
  (rust macOS/Ubuntu, gpu-contracts, media-encoding, apple-silicon-preview) läpäisivät.
- ORK:n pistokokeet lähdekoodista samaa `main`ia vasten:
  `services/head-pose/main.swift` tuo `Vision`in ja ajaa `VNDetectFaceLandmarksRequest`in;
  `motion_lipsync::Viseme` on 13 tilaa; `ModelPolicy`/`no_ai`-hakuja ei löydy `crates`- ja
  `apps`-hakemistoista; `rules.rs`: `MAX_RULES = 64`, `MAX_RULE_OUTPUTS_PER_SCENE = 128`;
  workspace-manifestien normaali crate-graafi on syklitön (tarkistettu manifesteista).
- Pistokokeet eivät toista A01:n `cargo tree`- tai inventaarioajoja.

**Erot raportin ja koodin välillä (päätetty).**

| Ero | Päätös | Peruste |
| --- | --- | --- |
| egui vs. SwiftUI | egui/eframe jatkaa; ei SwiftUI-siirtoa | ADR-A01-01 |
| 10 vs. 13 viseemiä | 13 kanoninen; 10 vain mahdollisena häviöllisenä projektiona `motion-lipsync`issä. A07 voi esittää muutosta, mutta sarjallistettua enumia ei muuteta ilman uutta ORK-päätöstä | ADR-A01-08 |
| RETE-kirjasto vs. `expand_rules` | Oma toteutus; `rust-rule-engine`a ei lisätä. A03 laajentaa `rules.rs`:ää | ADR-A01-07 |
| `head-pose` (Apple Vision) | Tiukassa mallivapaassa polussa estetty; valinnaisessa polussa sallittu käyttäjän käynnistämänä syötteenä | ADR-A01-06 |
| No AI -politiikka | `ModelPolicy` + `ModelBackedStep` `motion-project`iin (ehdotus, ei toteutettu); toteutus G2:ssa, ks. ORK-G1-06 | ADR-A01-03, -09 |

**Päällekkäisyys automaattiohjauksen sarjan (01–10) kanssa.** Automaattiohjauksen
suunnitelmat (`ScriptIntent`, `DirectionPlan`, `AudioPlan`, `ShotPlan`, `ActingPlan`) ovat
adaptereita, jotka tuottavat nykyisiä `Script`-, `ShotList`- ja `Event`-rakenteita. Ne eivät
omista eivätkä monista tämän sarjan sopimuksia: `AudioPlan v1` viittaa `AudioTimeline v1`:een
(ORK-G1-04), ja niiden 24 kHz:n näytepaikat käyttävät samaa näytekelloa. Automaattiohjauksen
agentit 06 (puhe) ja 10 (integraatio) sopivat rajapintamuutokset orkestroijan kautta.

**Ehdot ja siirrot.**

- A01:n kaksi avointa kohtaa eivät estä G1:tä, mutta estävät G4:n: wheel-lukitus → KS-006,
  lisenssitekstit ja NOTICEt → KS-007. A01 merkitään `done` G0-laajuudessa.
- Kirjattu poikkeus: A02 aloitti G1-työn (PR #102, lähtö `496a8a4`) ennen G0-hyväksyntää.
  Työ otetaan G1:n syötteeksi, koska se noudattaa A01-päätöksiä eikä luo TrackSpec- tai
  EventLog-tyyppiä. PR #102 on konfliktissa nykyisen `main`in kanssa; A02 tekee rebasen
  ennen katselmusta. Ei yhdistetä ilman käyttäjän lupaa.

## ORK-G1 — Sopimusten omistajat (27.10.2026)

Päätökset perustuvat nykyiseen crate-graafiin (`main` `331f9d4`):
`motion-script → {motion-animation, motion-rig, motion-lipsync}`,
`motion-lipsync → motion-audio`, `motion-scene → {motion-animation, motion-rig, motion-script, motion-lipsync}`,
`motion-render → {motion-art, motion-rig, motion-scene, motion-lipsync, motion-script, motion-project, motion-animation}`,
`motion-project → {motion-art, motion-script}`, `motion-voice → {motion-script, motion-tts, motion-audio, motion-jobs, motion-lipsync, motion-scene}`.
`motion-audio` ja `motion-rig` eivät riipu muista workspace-crateista. Yksikään päätös ei
lisää uutta cratea. Ainoa sallittu uusi riippuvuusreuna on `motion-scene → motion-audio`
(syklitön, koska `motion-audio` on lehti).

| Päätös | Sopimus | Omistava crate | Kirjoittaja | Sijainti | Kuluttajat |
| --- | --- | --- | --- | --- | --- |
| ORK-G1-01 | SceneSpec | `motion-script` | A02 | `src/compiler.rs` (PR #105), `Script`/`Scene`/`Event` `src/lib.rs`issä | A03, A04, A09, automaattiohjaus 01 |
| ORK-G1-02 | TrackSpec (kirjoitettu + ratkaistu) | kirjoitettu: `motion-script`; ratkaistu: `motion-scene` | A02 (skeema), A04 (semantiikka ja ratkaisu) | `motion-script/src/lib.rs`; uusi `motion-scene/src/tracks.rs` | A05, A06, A08, A10 |
| ORK-G1-03 | EventLog v1 | `motion-script` | A03 | uusi `motion-script/src/event_log.rs` | A04, A06, A07, A09 |
| ORK-G1-04 | Näytekello + AudioTimeline v1 | kello: `motion-audio`; AudioTimeline: `motion-lipsync` | A07 | `motion-audio/src/lib.rs` (tai uusi `clock.rs`); uusi `motion-lipsync/src/audio_timeline.rs` | A04, A08, A09, A10, automaattiohjaus 06 |
| ORK-G1-05 | Rig-kiinnitys, tartuntapiste, nivelrajat, IK, esineluettelo | `motion-rig` | A05 | `motion-rig/src/definition.rs` ja uudet moduulit (esim. `attachment.rs`, `ik.rs`, `props.rs`) | A06, A08, A09 |
| ORK-G1-06 | Renderöintisopimus | `motion-render` | A08 | `motion-render/src/lib.rs`, `scene_commands.rs` | A09, A10 |

### ORK-G1-01 — SceneSpec omistaa `motion-script` (A02)

- SceneSpec ei ole uusi tyyppi: se on `motion_script::compiler::Compilation`
  (`script: Script`, `origins`) PR #102:n mukaisesti ja `Script::validate`n hyväksymä
  `Script`. Versio: MotionScript schedule `version: 1`. Vanha versioton `parse` säilyy.
- `motion-scene`n `ValidatedScript` on arvioinnin syöteraja. A04 ei lisää rinnakkaista
  kohtauskuvausta.
- Peruste: jäsennin, skeema ja validointi ovat jo `motion-script`issä (AGENTS.md:
  ei kilpailevaa jäsennintä).

### ORK-G1-02 — TrackSpec: kirjoitettu muoto `motion-script`, ratkaistu muoto `motion-scene`

- Kirjoitettu raitamuoto pysyy nykyisinä `Event`- ja animaatioratarakenteina `motion-script`issä.
  Jos A04 tarvitsee uuden kentän (esim. Rel/Abs/Revert-tila), A04 kirjaa vaatimuksen ja
  A02 tekee skeemamuutoksen; A04 ei kirjoita `motion-script`iin.
- Ratkaistut raidat (`{entiteetti, ominaisuus} → {from, to}`-välit, päällekkäisyyssääntö)
  ovat arvioitua tilaa ja kuuluvat `motion-scene`en (`tracks.rs`, A04). Niitä ei
  sarjallisteta dokumenttiformaatiksi.
- Peruste: dokumenttitila ja arvioitu tila erotetaan (AGENTS.md); `motion-scene` riippuu jo
  `motion-script`istä, joten uusia reunoja ei tarvita.

### ORK-G1-03 — EventLog omistaa `motion-script` (A03)

- Uusi moduuli `motion-script/src/event_log.rs`. Tapahtuman aika on kokonaislukuinen
  näytepaikka ORK-G1-04:n kellossa (kohtauksen tunnus + kohtauspaikallinen näyte), ei f64-sekunti.
- Peruste: `expand_rules` (säännöt) kuluttaa tapahtumat ja on samassa cratessa; `motion-scene`,
  `motion-project`, `motion-export` ja `studio-desktop` riippuvat jo `motion-script`istä.
  `motion-project` ei käy omistajaksi, koska `motion-project → motion-script` tekisi
  `rules.rs`:n käytöstä syklin. `motion-animation` (A06) ei saa riippua `motion-script`istä
  (sykli); vapautustapahtumat välitetään sille tavallisina arvoina `motion-scene`n kautta.
- Projektitiedoston upotus (`motion-project`) ja tallennus käyttöliittymästä ovat A09:n
  G3-työtä; A03 määrittää vain tyypin, sarjallistuksen ja validoinnin.
- Kirjoittajasääntö: `motion-script/src/lib.rs`in kirjoittaja on A02. A03 lisää siihen vain
  `pub mod event_log;` -rivin ja tarvittavat uudelleenviennit, PR #102:n yhdistämisen tai
  rebasen jälkeen.

### ORK-G1-04 — Näytekello `motion-audio`, AudioTimeline `motion-lipsync` (A07)

- Yksi kanoninen dialogin näytetaajuus (24 000 Hz) ja muunnokset sekunti ↔ näyte ↔ ruutu
  `motion-audio`ssa. Pyöristyssäännön ja kohtausoffsetit kirjaa A04 (sen tehtävälistan
  mukaisesti); A07 toteuttaa ne `motion-audio`ssa. Nykyiset kopiot
  (`motion-lipsync::english::visemes::DIALOGUE_SAMPLE_RATE`, `motion-scene::sequence::SEQUENCE_SAMPLE_RATE`,
  `motion-script::auto_direction::SAMPLE_RATE`, `motion-voice::export::SAMPLE_RATE`,
  `motion-export/tests/support::SAMPLE_RATE`) siirretään käyttämään sitä kunkin cratea
  omistavan agentin työssä; uusia kopioita ei lisätä.
- `AudioTimeline v1` `motion-lipsync/src/audio_timeline.rs`: lähde (tuotu / syntetisoitu),
  näytetaajuus, kesto, offset ja rajaus näytteinä (`motion_audio::MixClip`-sijoittelun kanssa
  yhtenevästi), viseemivälit ja `AlignmentSource`.
  Synteesin alkuperä (provider, malli- ja äänirevisio) tallennetaan datana; `motion-lipsync`
  ei saa riippua `motion-tts`:stä.
- Peruste: tyyppi tarvitsee `Viseme`n ja `AlignmentSource`n (`motion-lipsync`) ja
  sijoittelun (`motion-audio`, jo riippuvuus). `motion-scene` riippuu jo `motion-lipsync`istä,
  joten suun arviointi lukee aikajanan ilman uusia reunoja. `motion-voice` ei käy omistajaksi,
  koska `motion-voice → motion-scene` estäisi arvioijaa lukemasta tyyppiä.
- Viseemijoukko: 13 kanoninen (ORK-G0).

### ORK-G1-05 — Rig-kiinnitys omistaa `motion-rig` (A05)

- Juuri- ja lapsinivelet (lantio → jalat, rinta/hartiat → kädet), nivelrajat, kahden luun IK ja
  esineen `grip`-sidonta käden päätenivelen maailmamuunnokseen ovat `motion-rig`issä.
- Esineluettelon jäsennys ja tyypit siirretään `apps/studio-desktop/src/hand_props.rs`:stä
  `motion-rig`iin (esineet ovat jo `RigidPartDefinition`-osia). Esinedata
  (`examples/stick-studio/hand-props.json`) ja kuvat pysyvät aineistona; `motion-art` ei omista
  niveliä. `hand_props.rs`:n kirjoittaja on A05 G2-siirron ajan, sen jälkeen A09.
- IK-kohteen aikajanaraita noudattaa ORK-G1-02:ta (kirjoitettu muoto A02, ratkaisu A04).
- Peruste: `motion-rig` on lehti, ja kaikki kuluttajat (scene, render, export, desktop)
  riippuvat siitä jo; `motion-art` ei riipu `motion-rig`istä.

### ORK-G1-06 — Renderöintisopimus omistaa `motion-render` (A08)

- Syöte: `&motion_scene::EvaluatedScene` (tyypin omistaa A04), taide `motion-art`ista ja
  suun tila `motion-scene`n arvioimana. Tuloste: SVG (`render_svg*`) ja GPU:n
  `SceneCommands`. Renderöintifunktiot eivät ota aikaa parametrina eivätkä pidä kelloa.
- Syvyysjärjestys ja tasatilanteen sääntö dokumentoidaan ja testataan `motion-render`issä.
  `EvaluatedScene`n kenttämuutokset pyydetään A04:ltä.
- `motion-export` kutsuu samaa arvioijaa ja renderöijää; se ei laske aikaa eri tavalla.

### Yhteiset säännöt kaikille G1-sopimuksille

1. Jokainen sarjallistettu sopimus on versioitu (`version: 1` tai `format_version`); rikkova
   muutos vaatii uuden version ja migraation.
2. Kirjoitettu aika on äärellisiä f64-sekunteja kohtauksen alusta (A02). EventLog,
   AudioTimeline ja ratkaistut raidat käyttävät kokonaislukuisia näytepaikkoja yhdessä kellossa.
3. Rajapintamuutos sovitaan orkestroijan kautta ja avaa kuluttajien tarkistukset uudelleen.
4. Yksi kirjoittaja tiedostoa kohden; omistajan ulkopuolinen muutos tehdään pyyntönä omistajalle.
5. `ModelPolicy` (ADR-A01-09) pysyy `motion-project`issä. Tyypin toteuttaa A07 G2:ssa
   (uusi `motion-project/src/model_policy.rs`); `ProjectDocument`-kenttä ja
   käyttöliittymän tarkistuskohdat ovat A09:n.

### G1:n hyväksyntäehto

G1 hyväksytään vasta, kun (a) PR #102:n työ on rebasettu ja katselmoitu (PR #105, yhdistetty `d9db5be`), (b) A03:n EventLog v1 ja
(c) A07:n AudioTimeline v1 (täyttyy: ORK-G1-KS-002d) ovat versioituja, sarjallistuvia ja testattuja, (d) A04 on kirjannut
TrackSpec-sopimuksen ja ajan muunnossäännöt, ja (e) jokaisella on kelvollinen ja virheellinen
esimerkki. Vasta sen jälkeen G2-tehtävät merkitään `ready`-tilaan.

## ORK-G1-KS-002d — A07:n G1-osuuden katselmus: **hyväksytty** (27.10.2026)

**Näyttö.**

- PR #109 yhdistetty `main`iin (`ae39c4d`) käyttäjän nimenomaisella luvalla; kirjattu
  `master-progress.md`hen (`da5d8d7`). PR:n CI `62f96be`:llä 12/12 läpäisi.
- Ennen yhdistämistä haara yhdistettiin paikallisesti `main`iin (sis. A04:n PR #107):
  `cargo fmt --all --check` ok, `cargo clippy --workspace --all-targets` 0 varoitusta,
  `cargo test --workspace` 906 läpäisi, 0 epäonnistui, 8 ohitettu.
- ORK:n pistokokeet `main`issa `ae39c4d`: `AUDIO_TIMELINE_FORMAT_VERSION = 1`,
  `deny_unknown_fields`; `cargo test -p motion-lipsync --test audio_timeline_examples`
  (2/2), `--lib audio_timeline` (5/5) ja `cargo test -p motion-voice --test model_free`
  (3/3) läpäisivät; `examples/audio-timeline-v1/` sisältää 2 kelvollista ja 4
  virheellistä esimerkkiä.

**Arvio G1-ehtoa (c) ja (e) vasten.**

| Ehto | Tulos |
| --- | --- |
| Versioitu | `format_version: 1`; muu versio on virhe (testi + esimerkki) |
| Sarjallistuva | tiukka JSON, edestakainen muunnos identtinen (testit) |
| Testattu | validointi, sijoittelu = `MixClip`, rajaus, sekoitus rajoilla, mallivapaa polku 0 kutsua |
| Kelvollinen ja virheellinen esimerkki | fi `manual`, en `estimated`; päällekkäisyys, rajaus, versio, tuntematon IPA |
| Omistajuus ORK-G1-04 | kello `motion-audio/src/clock.rs`, aikajana `motion-lipsync/src/audio_timeline.rs`; `motion-lipsync` ei riipu `motion-tts`:stä |
| Uudet riippuvuusreunat | vain dev-riippuvuus `motion-voice → motion-project` (testit); normaali graafi ennallaan |

**Rajaukset ja siirrot.** Hyväksyntä koskee vain G1-sopimusta. Suun ajoitusta ei ole
mitattu oikeaa puhetta vasten. Ihmisen äänittämät esimerkit ja AudioTimelinen käyttö
`motion-scene`n suun arvioinnissa siirretään KS-002e:hen (G2). `ModelPolicy`n tallennus
`ProjectDocument`iin ja käyttöliittymän tarkistuskohdat ovat A09:n (G2/G3);
`motion-export`in tarkistus A10:n; kellokopioiden delegointi A04/A02/A10:n; sekoituksen
käyttö renderöijässä A08:n. G1 kokonaisuutena odottaa vielä KS-002a/b/c:n ORK-katselmusta.

## ORK-G1 — Sopimusportti: **hyväksytty ehdoin** (27.10.2026)

**Näyttö.** Kaikki neljä G1-sopimusta on yhdistetty `main`iin, ja jokaisen PR:n CI läpäisi
12/12: PR #105 (A02, `d9db5be`), PR #106 (A03, `6ddfdcc`), PR #107 (A04, `10135ad`) ja
PR #109 (A07, `ae39c4d`; ks. ORK-G1-KS-002d). ORK:n pistokokeet `main`issa `393c2f1`
(macOS, paikallisesti): `cargo test -p motion-script` 132 läpäisi, `-p motion-scene` 65,
`-p motion-export` 143 (8 ohitettu), 0 epäonnistui. Esimerkkikansiot luetaan testeissä:
`motionscript-v1` (`tests/compiler.rs`), `event-log-v1` (`tests/event_log.rs`),
`trackspec-v1` (`tests/timeline_determinism.rs`), `audio-timeline-v1`
(`tests/audio_timeline_examples.rs`).

| Ehto | A02 SceneSpec | A03 EventLog | A04 TrackSpec | A07 AudioTimeline |
| --- | --- | --- | --- | --- |
| (a) katselmoitu ja yhdistetty | PR #105 (korvaa #102) | PR #106 | PR #107 | PR #109 |
| Versio | schedule `version: 1` (`compiler::VERSION`) | `version: 1` (`EVENT_LOG_VERSION`) | ratkaistua muotoa ei sarjallisteta (ORK-G1-02); kirjoitettu muoto = A02:n versio | `format_version: 1` |
| Sarjallistus ja validointi | YAML, paikannetut tyypitetyt virheet | JSON, `deny_unknown_fields`, rajat | – (arvioitu tila) | JSON, `deny_unknown_fields`, tyypitetyt virheet |
| Testattu | kääntäjä, primitiivit, virhetyypit | toisto → sama laukaisujärjestys | aikahyppy = ruuduittainen toisto bitilleen | 0 providerikutsua tiukassa polussa |
| (e) kelvollinen + virheellinen esimerkki | 1 + 3 | 1 + 1 | 1 + 1 | 2 + 4 |
| Yksi näytekello (yhteinen sääntö 2) | – | **poikkeama**: `sample_rate` 1…192 000 hyväksytään | 24 kHz (`motion_scene::time`) | 24 kHz (`motion_audio::clock`) |

**Poikkeama ja ehto.** ORK-G1-03 ja yhteinen sääntö 2 edellyttävät, että EventLogin
näytepaikat ovat ORK-G1-04:n kellossa. `EventLog::validate` hyväksyy kuitenkin minkä
tahansa taajuuden väliltä 1…192 000 Hz (`event_log.rs`, `MAX_EVENT_LOG_SAMPLE_RATE`).
Nykyinen kuluttaja (`rules.rs`) muuntaa näytteet sekunneiksi lokin omalla taajuudella,
joten tulos on johdonmukainen eikä virheellinen. Muulla kuin 24 kHz:n taajuudella
tapahtumat eivät kuitenkaan osu kellon näytteisiin, ja poikkeava loki hyväksytään
hiljaa. Päätös: G1 hyväksytään, ja korjaus on **KS-002f** (A03): `sample_rate` on
24 000 Hz, muu taajuus on tyypitetty virhe, ja tästä lisätään virheellinen esimerkki.
KS-002f on tehtävä ennen kuin EventLogin G2-kuluttajat (A06 raajojen vapautus, A09
tallennus) käyttävät lokia.

**Muut siirrot (eivät estä G2:ta).** KS-002g: kellokopiot delegoimaan `motion_audio::clock`iin
(A04/A02/A10). KS-002e (A07): ihmisen äänittämät esimerkit ja AudioTimelinen käyttö suun
arvioinnissa. PR #102 (A02:n alkuperäinen, korvattu PR #105:llä) on yhä auki: ORK
suosittaa sen sulkemista käyttäjän luvalla. KS-006 ja KS-007 ovat edelleen G4:n ehtoja.

**G2 avataan.** A04 (G2-osuus), A05, A06, A07 (KS-002e) ja A08 ovat `ready`, ja jokaisella
on oma tiedostoalueensa (ks. G2-tarkistuslista). Rajapintamuutokset sovitaan edelleen
orkestroijan kautta.

## Hyväksymisehdot

- Jokaisella `done`-tehtävällä on linkitetty näyttö (commit, testiajo, CI-ajo, kuva tai raportti).
- Yksikään portti ei avaudu ennen edellisen portin hyväksyntää; poikkeukset kirjataan.
- Rekisterissä ei ole tehtävää ilman omistajaa, riippuvuuksia ja hyväksymisehtoa.

## Lopputulema

Odotettu: ajan tasalla oleva tehtävärekisteri, portti kerrallaan hyväksytyt sopimukset ja
toteutukset, käyttäjälle raportoitu tila (toteutettu / tarkistettu / estetty / seuraavaksi).

### Toteutunut lopputulema

### ORK G0/G1 — 27.10.2026
- Tila: running
- Omistaja: ORK
- Tehty: G0 hyväksytty (ORK-G0); G1-sopimusten omistavat cratet päätetty (ORK-G1-01…06);
  rekisteri, README:n tilataulukko ja agenttitiedostojen tilarivit päivitetty. Haara `docs/ork-g1`.
- Tarkistettu: PR #101/#103 yhdistetty ja niiden CI läpäisi; lähdekoodin pistokokeet
  (ks. ORK-G0); crate-graafi luettu manifesteista. Tuotantokoodia ei muutettu, joten
  `cargo`-tarkistukset eivät ole tämän muutoksen näyttöä.
- Tekemättä: G1-sopimusten versiot ja esimerkit (A02, A03, A04, A07); KS-006 ja KS-007 (G4).
- Estot ja riskit: PR #102 konfliktissa `main`in kanssa (A02); A04 odottaa A02:ta ja A03:a.
- Seuraavaksi: **A03** (EventLog v1, `motion-script/src/event_log.rs`). Rinnakkain
  voivat edetä A02 (PR #102:n rebase) ja A07 (näytekello + AudioTimeline v1), koska tiedostot
  eivät mene päällekkäin. A04 aloittaa, kun A02 on yhdistetty ja EventLog v1 on luonnoksena.

### ORK-G1-KS-002d — 27.10.2026
- Tila: running
- Omistaja: ORK
- Tehty: A07:n KS-002d katselmoitu ja hyväksytty G1-osuudelta (ORK-G1-KS-002d); KS-002c:n
  tila päivitetty vastaamaan yhdistettyä PR #107:ää; uusi KS-002e (A07, G2). Haara
  `docs/ork-a07-merge`.
- Tarkistettu: PR #109 yhdistetty `ae39c4d`, CI 12/12; yhdistetyn tuloksen paikalliset
  tarkistukset (906/0/8); pistokokeet `main`issa (ks. ORK-G1-KS-002d). Tämä muutos on
  pelkkää dokumentaatiota.
- Tekemättä: KS-002a/b/c:n G1-katselmus ja koko G1:n hyväksyntä; G2-tehtävien `ready`.
- Estot ja riskit: suun ajoitusta ei ole mitattu oikeaa puhetta vasten (KS-002e).
- Seuraavaksi: ORK katselmoi KS-002a/b/c ja päättää G1:stä; sen jälkeen G2 (A04–A08).

### ORK-G1 (portti) — 27.10.2026
- Tila: running
- Omistaja: ORK
- Tehty: KS-002a/b/c katselmoitu; G1 hyväksytty ehdoin (ORK-G1); KS-002 ja KS-002a/c ovat
  `done`, KS-002b `done` ehdoin; uudet KS-002f (A03, estää EventLogin G2-kuluttajat) ja
  KS-002g (kellokopiot); KS-003 ja A04–A08 `ready`. Haara `docs/ork-g1-gate`.
- Tarkistettu: PR #105/#106/#107/#109 yhdistetty, CI 12/12 jokaisessa; pistokokeet
  `main`issa `393c2f1` (motion-script 132, motion-scene 65, motion-export 143 läpäisi,
  0 epäonnistui); `event_log.rs`:n taajuusvalidointi luettu lähdekoodista. Tämä muutos
  on pelkkää dokumentaatiota.
- Tekemättä: KS-002f, KS-002g, KS-002e; PR #102:n sulkeminen (käyttäjän lupa); G2:n näyte.
- Estot ja riskit: EventLogin taajuuspoikkeama, kunnes KS-002f on tehty; suun ajoitusta
  ei ole mitattu oikeaa puhetta vasten.
- Seuraavaksi: A03 tekee KS-002f:n; rinnakkain G2: A04, A05, A07 ja A08 omilla
  tiedostoillaan, A06 A05:n kiinnityssopimuksen mukaan.

## Kopioitava tehtävänanto

> Toimi Kilsat Studion orkestroijana. Lue `AGENTS.md`, `docs/agents/README.md`,
> `docs/agents/00-orkestroija.md` ja raportin luvut 6–10. Aloita nykytilan tarkistuksesta
> ja päivitä tehtävärekisteri. Sovi A01:n kanssa moduulirajat ja sopimukset ennen
> toteutusta. Anna jokaiselle agentille sen tehtävätiedosto, sallittu muutosalue,
> riippuvuudet ja hyväksymisehto. Käynnistä rinnakkain vain riippumattomat työt, yksi
> kirjoittaja tiedostoa kohden. Älä merkitse tehtävää valmiiksi pelkän raportin perusteella.
