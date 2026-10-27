# ORK — Orkestroija

Tila: **running** · Portit: G0–G4 · Lähtö: commit `496a8a4` · Viimeisin päätös: ORK-G1 (portti hyväksytty ehdoin) ja ORK-G2-01 (tiedostojako), 27.10.2026 (`main` `393c2f1`)

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
| KS-002 | A02, A03, A04, A07 (+A05/A08) | KS-001 | G1: SceneSpec/TrackSpec/EventLog/AudioTimeline-sopimukset | done (ORK-G1, hyväksytty ehdoin; ehdot KS-002f ja KS-002g) |
| KS-002a | A02 | KS-001 | SceneSpec: versioitu kääntäjä, PR #105 (korvaa #102) | done (PR #105 `d9db5be`; ORK-G1) |
| KS-002b | A03 | KS-002a | EventLog v1 ja sääntösopimus | done (PR #106 `6ddfdcc`; ORK-G1, korjaus KS-002f) |
| KS-002c | A04 | KS-002a, KS-002b | TrackSpec-sopimus ja ajan muunnossäännöt | done (PR #107 `10135ad`; ORK-G1-A04, ehto A07:n kellolle) |
| KS-002d | A07 | KS-001 | AudioTimeline v1 ja yhteinen näytekello | done (G1-osuus; PR #109 `ae39c4d`, ORK-G1-KS-002d) |
| KS-002e | A07 | KS-002d, KS-003 | Ihmisen äänittämät fi/en-WAV-esimerkit ja niitä vasten tarkistettu ajoitus; AudioTimelinen käyttö suun arvioinnissa | todo (G2) |
| KS-002f | A03 | KS-002b, KS-002d | G1-palautus: EventLog v1 hyväksyy vain kanonisen kellon `sample_rate`n (24 000) | done (PR #113 `5e93709`) |
| KS-002g | A04 | KS-002c, KS-002d | G1-palautus: `motion_scene::time` delegoi `motion_audio::clock`iin (ORK-G1-A04, ehto 1) | done (PR #114 `3551257`; A02:n ja A10:n kopiot → KS-002h) |
| KS-002h | A02 (+A10) | KS-002d | `motion_script::auto_direction::SAMPLE_RATE` kanonisen kellon aliakseksi; `motion-export`in testituen kellokopio (A10, ORK-G1/PR #112) | todo (ei estä G2:ta) |
| KS-003 | A04–A08 | KS-002 | G2: pienin toimiva kohtaus | running (KS-002f/g, KS-003a/b yhdistetty; KS-003c puhelinta lukuun ottamatta; KS-003d ready) |
| KS-003a | A05 | KS-002 | Kiinnityssopimus, nivelrajat, kahden luun IK, `grip`-sidonta, esineluettelo `motion-rig`iin | done (PR #117 `824c399`) |
| KS-003b | A06 | KS-002 | `spring`-moduuli: analyyttiset jouset, voimat, profiilit, vapautuksen alkuarvot | done (PR #115 `974ef76`; Dangle-ehdotus odottaa A05:n kommenttia) |
| KS-003c | A08 | KS-002; puhelinkohtaus myös KS-003a | Renderöintisopimus, syvyysjärjestys, rajaussäännöt, testikohtaus ja vertailukuvat | osittain done (PR #116 `9c69902`); puhelinkohtaus ja sen vertailukuvat auki, `grip` nyt mainissa |
| KS-003d | A04 | KS-002g, KS-003a, KS-003b | `EvaluatedScene`-pyynnöt A05/A08:lta, IK-kohteen ja vapautuksen ratkaisu `motion-scene`ssä | ready (avausehto täyttyi: KS-002g, KS-003a ja KS-003b yhdistetty 27.10.2026) |
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
- [x] Hyväksy yhteinen versio ja esimerkit; kirjaa ne `docs/agents/`-tiedostoihin. → ORK-G1.
- [x] Merkitse G2-tehtävät `ready`-tilaan vasta sopimusten hyväksynnän jälkeen. → ORK-G2-01.

### G2 — Pienin toimiva kohtaus
- [x] Jaa tiedostot ilman päällekkäisiä kirjoittajia (A04 motion-scene, A05 motion-rig,
      A06 motion-animation-fysiikkamoduuli, A07 ääni-cratet, A08 motion-render/motion-art).
      → ORK-G2-01 (A05, A06, A08; A04:n ja A07:n G2-osuudet avataan erikseen).
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

## ORK-G1-A04 — KS-002c katselmus: **hyväksytty ehdolla** (27.10.2026)

**Näyttö.** PR #107 yhdistetty `main`iin `10135ad`; CI `c95fbd9`:llä 12/12 läpi;
paikallisesti macOS:llä neljä cargo-tarkistusta ok (884 testiä, 0 epäonnistunutta);
kirjaus `master-progress.md` (`dc9079e`).

| Ehto | Tulos | Näyttö |
| --- | --- | --- |
| G1 (d): TrackSpec-sopimus ja ajan muunnossäännöt | täyttyy | [editable-timeline.md § TrackSpec](../editable-timeline.md#trackspec-overlap-order-and-time-conversion-a04) |
| G1 (e): kelvollinen ja virheellinen esimerkki | täyttyy (lisätty katselmuksessa) | `examples/trackspec-v1/`, testit lukevat ne |
| A04: aikahyppy ≡ toisto sovitulla toleranssilla | täyttyy, bitti-identtinen | `motion-scene/tests/timeline_determinism.rs` |
| A04: Rel/Abs/Revert ja päällekkäisyys dokumentoitu ja testattu | täyttyy | `src/tracks.rs`-testit, vastaavuustaulukko |
| A04: nykyiset motion-scene- ja motion-export-testit | täyttyy | `cargo test --workspace`, CI |
| ORK-G1-02: ratkaistu muoto `motion-scene`ssä, ei sarjallisteta | täyttyy | `src/tracks.rs` |
| Yksi kirjoittaja | täyttyy | muutokset vain `motion-scene`, `docs/editable-timeline.md`, A04-tiedosto, `examples/trackspec-v1/` |
| Ei toista kelloa tai arvioijaa | täyttyy ehdolla | ks. ehto 1 |

**Ehdot ja poikkeamat.**

1. `motion_scene::time` nimeää nykyisen 24 kHz:n kellon säännöt `motion-scene`ssä, kun
   ORK-G1-04 sijoittaa kanonisen kellon `motion-audio`on. Hyväksytään väliaikaisena, koska
   vakio oli jo `motion-scene`ssä eikä uutta kopiota lisätty. Kun A07:n kello yhdistetään,
   A04 vaihtaa `motion_scene::time`n delegoimaan siihen (reuna `motion-scene → motion-audio`
   on sallittu); A07:n toteutuksen on käytettävä samoja sääntöjä tai muutos sovitaan ORK:n kautta.
2. Yhteinen sääntö 2 (ratkaistut raidat kokonaislukunäytteinä): ratkaistut välit kantavat
   alku- ja loppunäytteen, mutta interpoloivat kirjoitetuilla f64-sekunneilla, jotta nykyiset
   tulokset säilyvät. Hyväksytty; näytteet ovat kohdistusta varten (EventLog, AudioTimeline).
3. Vain `Event::Move` käyttää `ResolvedTrack`ia. Muiden ominaisuuksien säännöt on kirjattu ja
   testattu; siirto ei ole G1-ehto. Kirjoitettu `Rel`/`Revert` on A02:n skeemapyyntö (ei G1-ehto).

**Vaikutus porttiin.** KS-002c `done`. G1 ei vielä avaudu: (c) A07:n AudioTimeline v1
(KS-002d) puuttuu, ja ORK:n G1-katselmus KS-002a/KS-002b:lle on yhä auki. G2-tehtäviä ei
merkitä `ready`-tilaan.

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

## ORK-G1 — Porttipäätös: **hyväksytty ehdoin** (27.10.2026)

**Näyttö.**

- Yhdistetyt PR:t ja CI (kaikki 12/12 läpi PR:n viimeisellä commitilla): #105 A02 (`f2eb102`,
  yhdistetty `d9db5be`), #106 A03 (`2352022` → `6ddfdcc`), #107 A04 (`c95fbd9` → `10135ad`),
  #109 A07 (`62f96be` → `ae39c4d`); #110 ORK (11/11).
- ORK:n paikalliset tarkistukset `main` `393c2f1`:llä (macOS, Apple Silicon):
  `cargo fmt --all --check` ok; `cargo check --workspace` ok;
  `cargo clippy --workspace --all-targets` 0 varoitusta; `cargo test --workspace`
  906 läpäisi, 0 epäonnistui, 8 ohitettu.
- Lähdekoodin luku: `motion_audio::clock` ja `motion_scene::time` (sama pyöristys, sama
  ruutu ↔ näyte -sääntö, rivi riviltä), `event_log.rs` (versio, `sample_rate`-validointi),
  `audio_timeline.rs` (`format_version`, `sample_rate` = 24 000), testien esimerkkiviittaukset.

**Sopimukset G1-ehtoja (a)–(e) vasten.**

| Sopimus | Versio | Sarjallistus | Esimerkit (kelvollinen / virheellinen), testit lukevat | Arvio |
| --- | --- | --- | --- | --- |
| SceneSpec (A02, ORK-G1-01) | `version: 1`, JSON Schema `const: 1`; versio 2 → `$.version`-virhe | YAML v1-kuori, vanha versioton `parse` säilyy | `examples/motionscript-v1/`: `nested.yaml` / 3 virheellistä (`tests/compiler.rs`) | hyväksytty |
| TrackSpec (A04, ORK-G1-02) | kirjoitettu muoto = SceneSpec v1; ratkaistua ei sarjallisteta | – | `examples/trackspec-v1/` 1 / 1 (`tests/timeline_determinism.rs`) | hyväksytty (ORK-G1-A04) |
| EventLog v1 (A03, ORK-G1-03) | `version: 1`; muu versio tyypitetty virhe | tiukka JSON, edestakainen muunnos | `examples/event-log-v1/` 1 / 1 (`tests/event_log.rs`) | hyväksytty, **puute 1** |
| Näytekello + AudioTimeline v1 (A07, ORK-G1-04) | `format_version: 1` | tiukka JSON | `examples/audio-timeline-v1/` 2 / 4 | hyväksytty (ORK-G1-KS-002d) |

**Yhtenäisyys.** Kirjoitettu aika on äärellisiä f64-sekunteja (SceneSpec); TrackSpec,
EventLog ja AudioTimeline kohdistavat kokonaislukunäytteillä. Muunnossäännöt (sekunti →
lähin näyte, puolikkaat nollasta poispäin; `floor(frame · 24000 / fps)`; näytteen näyttää
viimeinen ruutu, jonka ensimmäinen näyte ei ole sen jälkeen) ovat samat A04:n ja A07:n
toteutuksissa. Kaksi puutetta palautetaan omistajille:

1. **EventLog ei ole sidottu kanoniseen kelloon (A03 → KS-002f).** ORK-G1-03 määrää tapahtuman
   ajan ORK-G1-04:n kellossa, mutta `EventLog::validate` hyväksyy `sample_rate`n 1…192 000 ja
   dokumentti siirtää vertailun kuluttajille, joita ei ole. AudioTimeline hylkää muun kuin
   24 000 Hz:n. Korjaus: hylkää muu kuin `DIALOGUE_SAMPLE_RATE` tyypitetyllä virheellä,
   lisää virheellinen esimerkki (`examples/event-log-v1/invalid-sample-rate.json`) ja testi,
   päivitä `docs/animation-rules.md` § EventLog v1. Vakio luetaan `motion_lipsync`in
   uudelleenviennistä (`motion_lipsync::english::DIALOGUE_SAMPLE_RATE`, alias
   `motion_audio::clock`iin), joten uutta riippuvuusreunaa tai `Cargo.lock`-muutosta ei tarvita.
   Tiedostot: `crates/motion-script/src/event_log.rs`, `crates/motion-script/tests/event_log.rs`,
   `examples/event-log-v1/**`, `docs/animation-rules.md`, A03-tiedosto.
2. **Kaksi kellototeutusta (A04 → KS-002g, ORK-G1-A04 ehto 1).** A07:n kello on yhdistetty,
   joten ehto on lauennut. Säännöt ovat nyt identtiset, mutta kahdesta kopiosta voi tulla
   kaksi kelloa. Korjaus: `motion_scene::time` delegoi `motion_audio::clock`iin säilyttäen
   julkiset polut (`motion_scene::SEQUENCE_SAMPLE_RATE` = alias) ja virhetyypit; lisää reuna
   `motion-scene → motion-audio` (sallittu ORK-G1:ssä). Tiedostot:
   `crates/motion-scene/src/time.rs`, `crates/motion-scene/Cargo.toml`, `Cargo.lock`, A04-tiedosto.

`motion_script::auto_direction::SAMPLE_RATE` (A02) ja `motion-export/tests/support`in kopio
(A10) ovat samaa arvoa kantavia vakioita ilman omia muunnossääntöjä; ne siirretään
(KS-002h, A10:n G3-työ) eivätkä estä G2:ta.

**Päätös ja poikkeus.** G1 hyväksytään ehdoin. Puutteet koskevat EventLogin ja kellon
sisäistä yhtenäisyyttä, eivät sopimuksia, joita A05, A06 ja A08 kuluttavat (`motion-rig`,
`EvaluatedScene`, renderöintisopimus), joten G2:n riippumattomat työt avataan nyt. Kirjattu
poikkeus säännöstä "portti avautuu vasta edellisen hyväksynnän jälkeen": KS-002f ja KS-002g on
yhdistettävä ennen kuin (i) mikään G2-työ lukee EventLogin näytteitä tai `motion_scene::time`a
uusissa kutsupaikoissa ja (ii) ORK hyväksyy G2:n (KS-003). Jos korjaus muuttaa sopimuksen
muotoa, kuluttajien tarkistukset avataan uudelleen (yhteinen sääntö 3).

## ORK-G2-01 — G2:n tiedostoalueet A05, A06 ja A08 (27.10.2026)

Pohja: `main` `393c2f1` (tai uudempi, kun tämä PR on yhdistetty). Kukin agentti työskentelee
omassa haarassaan ja tekee oman PR:n. Yksi kirjoittaja tiedostoa kohden; muiden tiedostojen
tarvitsemat muutokset tehdään pyyntönä omistajalle ORK:n kautta. Taulukko on tyhjentävä:
mitä siinä ei ole annettu agentille, on sille vain luettavaa.

| Agentti | Tehtävä | Haara | Kirjoittaa (yksinoikeus G2:n ajan) |
| --- | --- | --- | --- |
| A05 | KS-003a | `feat/a05-rig-ik` | `crates/motion-rig/**` (sis. `Cargo.toml`, uudet `src/attachment.rs`, `src/ik.rs`, `src/props.rs`, `tests/`); `crates/motion-animation/src/clips.rs`; uusi `crates/motion-animation/tests/limb_attachment.rs`; `apps/studio-desktop/src/hand_props.rs` (ohueksi kääreeksi, julkinen rajapinta `main.rs`:lle ennallaan); `examples/stick-studio/hand-props.json`; uusi `examples/g2-rig/**`; `docs/custom-rigid-characters.md`, `docs/rig-outlines.md`, uusi `docs/rig-attachment-ik.md`; `docs/agents/A05-riggaus-ik-esineet.md` |
| A06 | KS-003b | `feat/a06-springs` | uusi `crates/motion-animation/src/spring.rs`; `crates/motion-animation/src/lib.rs` vain rivit `pub mod spring;` ja sen uudelleenviennit; uusi `crates/motion-animation/tests/spring.rs`; uusi `docs/physics-springs.md`; `docs/agents/A06-fysiikka-vedettavat.md` |
| A08 | KS-003c | `feat/a08-render-camera` | `crates/motion-render/**` (sis. `tests/golden/`); `crates/motion-art/**`; `docs/composition.md`, `docs/camera-presets.md`, `docs/art-rendering.md`, uusi `docs/render-contract.md`; uusi `docs/evidence/g2-render/**`; uusi `examples/g2-minimal-scene/**`; `docs/agents/A08-renderointi-kamera.md` |

**Rajat ja rajapinnat.**

- `crates/motion-animation`: A05 kirjoittaa vain `clips.rs`:ää, A06 vain `spring.rs`:ää ja
  `lib.rs`:n `spring`-rivejä. Muu `lib.rs` (easing, `Track`) jäädytetään G2:n ajaksi;
  muutostarve pyydetään ORK:lta. `motion-animation/Cargo.toml`ia ei muuteta.
- `motion-animation` ei saa riippua `motion-script`istä (ORK-G1-03). A06:n vapautusrajapinta
  ottaa tavallisia arvoja (aika, sijainti, nopeus tai näyteikkuna); EventLog → jousen alkuarvot
  -muunnos on A04:n `motion-scene`-työtä (KS-003d). Dangle-niveljoukko kirjataan ehdotuksena
  `docs/physics-springs.md`:hen; rig-kentät sille tekee A05 pyynnöstä.
- A05:n IK on puhdas funktio (kohde arvona). IK-kohteen aikajanaraita noudattaa ORK-G1-02:ta:
  kirjoitettu muoto on A02:n skeemapyyntö, ratkaisu A04:n (KS-003d). A05 ei kirjoita
  `motion-script`iin eikä `motion-scene`en.
- A08 lukee `EvaluatedScene`ä ja `motion-rig`in rajapintoja; kenttämuutokset pyydetään A04:ltä
  (KS-003d). Testikohtauksen puhelinosuus käyttää A05:n `grip`-rajapintaa vasta, kun KS-003a:n
  PR on yhdistetty; sitä ennen A08 tekee sopimuksen, syvyysjärjestyksen, rajaussäännöt ja
  kuvat ilman puhelinta. A08 ei muuta `examples/stick-studio/**`ia, `motion-export`ia (A10)
  eikä `apps/studio-desktop/src/camera_framing.rs`:ää (A09).
- Kaikki: ei uusia crateja eikä ulkoisia riippuvuuksia ilman A01:n/ORK:n päätöstä.
  `Cargo.lock`ia ei muokata käsin; jos oma manifestimuutos muuttaa sitä, myöhemmin
  yhdistettävä PR rebasetaan ja `Cargo.lock` tuotetaan uudelleen `cargo check`illä.
  `docs/master-progress.md`, `docs/agents/README.md` ja `00-orkestroija.md` ovat ORK:n;
  agentit päivittävät vain oman tiedostonsa tarkistuslistan ja Toteutunut lopputulema -osion.
- Samaan aikaan käynnissä olevat palautukset eivät mene näiden alueiden päälle: KS-002f (A03)
  `motion-script`issä, KS-002g (A04) `motion-scene`ssä ja `Cargo.lock`issa.

**Hyväksymisehdot G2-luovutukselle.** Kunkin agentin tiedoston Hyväksymisehdot, neljä
cargo-tarkistusta työtilan juuresta, CI läpi PR:ssä ja luvun 10.2 yhteenveto. ORK tarkistaa
lisäksi, että PR:n `git diff --name-only` pysyy yllä olevalla alueella.

**Rinnakkaisuus.** A05, A06 ja A08 voivat alkaa heti ja rinnakkain. Ainoa järjestysriippuvuus
on A08:n puhelinkohtaus → KS-003a. A04:n (KS-003d) ja A07:n (KS-002e) G2-osuudet avataan
erillisellä ORK-päätöksellä.

## ORK-G1 (rinnakkainen kirjaus, PR #112) — Sopimusportti: **hyväksytty ehdoin** (27.10.2026)

*Huom. (yhdistetty 27.10.2026):* sama porttipäätös kirjattiin rinnakkain PR:ssä #112 ja
PR:ssä #111. Päätökset ovat yhtenevät (G1 hyväksytty ehdoin, KS-002f, G2 auki). Rekisteri,
tilarivit ja G2:n tiedostojako noudattavat yllä olevaa ORK-G1:tä ja ORK-G2-01:tä; tämä
kirjaus säilytetään näyttönä ja historiana.

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

### ORK-G1 (portti, rinnakkainen kirjaus PR #112) — 27.10.2026
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

### ORK-G1 / ORK-G2-01 — 27.10.2026
- Tila: running
- Omistaja: ORK
- Tehty: G1 hyväksytty ehdoin (ORK-G1); kaksi puutetta palautettu omistajille (KS-002f A03,
  KS-002g A04); KS-002a/b `done`; G2:n tiedostoalueet jaettu A05:lle, A06:lle ja A08:lle
  (ORK-G2-01) ja KS-003a/b/c merkitty `ready`. Avoimen PR #108:n A04-katselmus
  (ORK-G1-A04) otettu tähän haaraan. Haara `docs/ork-g2`.
- Tarkistettu: yhdistettyjen PR:ien CI (#105, #106, #107, #109: 12/12; #110: 11/11); neljä
  cargo-tarkistusta `main` `393c2f1`:llä (ks. ORK-G1); kellojen, EventLogin ja AudioTimelinen
  lähdekoodi ja esimerkkien testiviittaukset luettu. Tämä muutos on pelkkää dokumentaatiota.
- Tekemättä: KS-002f, KS-002g, KS-002h; KS-003d (A04) ja KS-002e (A07) avaamatta; G2-näyte.
- Estot ja riskit: G2 avattiin ennen G1-puutteiden korjausta (kirjattu poikkeus, ORK-G1).
  `Cargo.lock` voi konfliktoida A04:n ja A05:n välillä; ratkaisu rebase + uudelleentuotto.
- Seuraavaksi: A05 (KS-003a), A06 (KS-003b) ja A08 (KS-003c) rinnakkain; A03 (KS-002f) ja
  A04 (KS-002g) korjaukset rinnakkain niiden kanssa.

### ORK-G2 yhdistämiset — 27.10.2026
- Tila: running
- Omistaja: ORK
- Tehty: käyttäjän luvalla yhdistetty #111 (`5f659f0`, ORK-G1/ORK-G2-01; rinnakkainen #112-kirjaus
  sovitettu), #113 A03 KS-002f (`5e93709`), #114 A04 KS-002g (`3551257`), #115 A06 KS-003b
  (`974ef76`), #117 A05 KS-003a (`824c399`) ja #116 A08 KS-003c ilman puhelinta (`9c69902`).
  KS-003d merkitty `ready`. PR #108 jäi auki: sen sisältö (ORK-G1-A04) tuli mainiin #111:n
  mukana; sulkeminen odottaa käyttäjän lupaa.
- Tarkistettu: jokaisen PR:n CI 12/12 (#111 11/11) viimeisellä commitilla; #113, #114, #115,
  #116 ja #117 päivitettiin mainia vastaan ennen yhdistämistä (agenttitiedostojen tilarivien
  ristiriidat ratkaistu); #117:n `Cargo.lock` tarkistettu `cargo metadata --locked` ja
  `cargo check --workspace --locked`; #116 paikallisesti `cargo check --workspace --all-targets
  --locked` ja motion-render/motion-art/motion-rig-testit 99/0 uutta mainia vastaan.
- Tekemättä: A08:n puhelinkohtaus; KS-003d (A04); KS-002e (A07); KS-002h; G2-näyte.
- Estot ja riskit: yhteinen `CARGO_TARGET_DIR` aiheutti agenttien välisiä ristikkäisiä
  käännösvirheitä; seuraavalla kierroksella raskaat testiajot vuorotellen. Levytila on tiukka.
- Seuraavaksi: A08 puhelinkohtaus (`grip`), A04 KS-003d.

## Kopioitava tehtävänanto

> Toimi Kilsat Studion orkestroijana. Lue `AGENTS.md`, `docs/agents/README.md`,
> `docs/agents/00-orkestroija.md` ja raportin luvut 6–10. Aloita nykytilan tarkistuksesta
> ja päivitä tehtävärekisteri. Sovi A01:n kanssa moduulirajat ja sopimukset ennen
> toteutusta. Anna jokaiselle agentille sen tehtävätiedosto, sallittu muutosalue,
> riippuvuudet ja hyväksymisehto. Käynnistä rinnakkain vain riippumattomat työt, yksi
> kirjoittaja tiedostoa kohden. Älä merkitse tehtävää valmiiksi pelkän raportin perusteella.
