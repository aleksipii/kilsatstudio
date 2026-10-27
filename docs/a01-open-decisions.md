# A01 — Avoimien kohtien päätökset

27.10.2026. Lähtö: `main`, commit `561036c`. Haara: `docs/a01-open-decisions`.
Tila: **review**. Täydentää [A01-arkkitehtuuripäätöksiä](a01-architecture.md)
(ADR-A01-01…05). Tämä on lähdekoodiin, manifesteihin, Cargo-graafiin ja crates.io-
metatietoihin perustuva päätösdokumentti. Tuotantokoodia, riippuvuuksia tai
käyttöliittymää ei muutettu. Päätökset ovat sopimuksia muille agenteille, eivät
väitteitä toteutetusta toiminnasta.

## ADR-A01-06 — `services/head-pose` (Apple Vision) on mallipohjainen syöte

**Havainto koodista.** [`services/head-pose/main.swift`](../services/head-pose/main.swift)
tuo `Vision`-kehyksen ja ajaa `VNDetectFaceLandmarksRequest`-pyynnön kamerakuvalle
(`AVFoundation`). Applen kasvon- ja maamerkkitunnistus on käyttöjärjestelmän mukana
tuleva opetettu malli. Apuohjelma ei lataa malleja, käytä verkkoa eikä tallenna
kuvaa; se käynnistyy vain käyttäjän valinnasta
([`webcam_pose.rs`](../apps/studio-desktop/src/webcam_pose.rs), `Resources/bin/head_pose`).
Tulos on käyttäjän tarkistama luonnos, joka lisätään tavallisiksi pään avainruuduiksi.

**Päätös.**

| Polku | Luokitus | Perustelu |
| --- | --- | --- |
| Tiukka mallivapaa | **Estetään** (mittaus, jatkuva seuranta ja nauhoitus) | Ajaa ajonaikaisesti opetettua tunnistinta. Järjestelmäkirjasto, offline-käyttö tai mallilatauksen puuttuminen eivät tee siitä mallivapaata (ADR-A01-03: offline ≠ mallivapaa). |
| Sääntöpohjainen + valinnainen paikallinen ML-esikäsittely | **Sallittu** käyttäjän erikseen käynnistämänä syötelaitteena | Paikallinen, ei verkkoa tai latauksia, tulos tarkistetaan ennen käyttöä ja on kumottava muokkaus. |

**Jatkovaatimukset (A09, ei toteutettu).**

1. Tiukassa polussa käyttöliittymä ei käynnistä `head_pose`-prosessia; kameraosio
   kertoo syyn. Esto tehdään ennen prosessin käynnistystä, ei tuloksen jälkeen.
2. Nauhoituksesta lisättyjen avainruutujen alkuperä tallennetaan (esim. `camera_capture`)
   versionoidusti. Nykyään ne ovat tavallisia avainruutuja ilman alkuperämerkintää,
   joten tiukka polku ei voi todentaa vanhan projektin avainruutujen alkuperää.
   Ennen merkintää vanhoja projekteja ei saa merkitä No AI -tuotannoiksi tältä osin.
3. Valmiiden avainruutujen arviointi ja renderöinti eivät aja mallia, joten ne
   sallitaan molemmissa poluissa; alkuperä ilmoitetaan.

## ADR-A01-07 — Neljä ehdokaskirjastoa: ei lisätä

Lähde: crates.io API 27.10.2026 (uusin versio, lisenssikenttä, normaalit riippuvuudet).
Metatieto ei ole lisenssiauditointi; lähdekoodia ei luettu eikä käännetty.

| Ehdokas | Uusin / julkaistu | Lisenssi (ilmoitettu) | Ylläpito | API ja riippuvuudet | Nykyinen vastine | Päätös |
| --- | --- | --- | --- | --- | --- | --- |
| `broadsheet` | 0.3.0 / 2026-07-06, 9 versiota, 222 latausta | MIT | Nuori, viimeisin päivitys 2026-07-09 | "Newspaper-styled" selitysvideomoottori; oma tilaton aikajana ja tallennus; `macroquad ^0.4` | `motion-scene::evaluate` (yhteinen kello esikatselulle ja viennille), egui-kuori | **Hylätty.** Toisi kilpailevan aikajanan/kellon ja oman ikkuna- ja renderöintipinon (AGENTS.md kieltää kilpailevat kellot ja arvioijat). |
| `rust-rule-engine` | 1.21.9 / 2025-10-01, 93 versiota | MIT | Aktiivinen, päivitetty 2026-10-25 | RETE, eteen-/taaksepäin ketjutus, GRL-tekstikieli; `chrono`, `nom 8`, `rexile`, `serde_json`, `thiserror`, `log`; valinnaisesti `redis`, `tokio` | [`motion-script/src/rules.rs`](../crates/motion-script/src/rules.rs): deklaratiiviset, rajatut (64 sääntöä, 128 tulosta/kohtaus), syklittömät säännöt | **Korvataan omalla toteutuksella** (A03 laajentaa `rules.rs`:ää). GRL olisi toinen käsikirjoituskieli MotionScriptin rinnalle; RETE ei ole tarpeen kymmenille säännöille; nopea versiotahti lisää API-riskiä. |
| `animato` | 1.7.3 / 2026-05-07, 21 versiota | MIT OR Apache-2.0 | Aktiivinen, päivitetty 2026-10-21; MSRV 1.89 | `animato-core`; jouset, fysiikka, aikajana, GPU ja UI-sidokset valinnaisina osacrateina | `motion-animation` (käyrät, klipit) | **Korvataan omalla toteutuksella** (A06: analyyttinen vaimennettu jousi `motion-animation`-crateen). Uudelleenarvio vain, jos A06 osoittaa konkreettisen puutteen; silloin A01 tarkistaa determinismin (f32/f64), osacratet ja lisenssit. |
| `rusty_skelform` | 0.8.0 / 2025-07-08, 17 versiota | MIT | Päivitetty 2026-09-03; **ei repository-linkkiä** crates.io:ssa | SkelForm-formaatin ajonaikainen lukija; `serde`, `zip ^8.6` | `motion-rig` (hierarkia, osat, variantit); A05:n kahden luun IK | **Hylätty.** Alkuperä on jäljitettävä ennen harkintaa; formaattisidonnainen 0.x-rajapinta; PSD-tuontia tai verkkodeformaatiota ei ole todennettu tarpeeksi. |

Yhteinen sääntö: ehdokasta ei lisätä ilman uutta A01-päätöstä, joka sisältää lähdekoodin
lisenssitiedostot, transitiiviset riippuvuudet `cargo tree`-graafina, MSRV-yhteensopivuuden
ja determinismitestin. Raportin luvun 4.1 suorituskykylupauksia (100 000+ entiteettiä,
tween.wgsl) ei voi perustaa näihin kirjastoihin.

## ADR-A01-08 — Viseemijoukko: 13 kanonista, 10 vain mahdollisena projektiona (A07 päättää)

**Nykytila.** [`motion_lipsync::Viseme`](../crates/motion-lipsync/src/lib.rs) on 13 tilaa,
foneemipohjainen (IPA), sarjallistetaan `snake_case`-nimin välimuisteihin ja
aikajanoihin; renderöijä ja kohtaus käyttävät sitä suoraan. Käsin tehty suun ajoitus
tallennetaan Rhubarbin muotokirjaimina A–H, X (9 muotoa), jotka kuvataan 13 tilaan
(`rhubarb::shape_viseme`). Raportin luku 3.3 kuvaa 10 tilaa (Rive/Character Animator
-tyyli) **kirjainpohjaisesti**.

| Raportti (10) | Vastaa nykyisiä (13) | Ero |
| --- | --- | --- |
| 0 Neutral | `Rest` | — |
| 1 AEI | `A`, `E`, `I` | Raportti yhdistää kolme vokaalimuotoa |
| 2 BMP | `Closed` | — |
| 3 FV | `TeethLip` | — |
| 4 CDGKNRSTXYZ | `TongueUp`, `Sibilant`, `R`, `TongueTeeth`; k/g → `Rest` | Raportti yhdistää neljä muotoa; k/g/h ovat nykyisin `Rest` |
| 5 U | `U` | — |
| 6 ChJSh (+H) | `Sh`; h → `Rest` | H on raportissa Sh-ryhmässä |
| 7 L | `TongueUp` (yhdessä t/d/n kanssa) | Raportti erottaa L:n, nykyinen ei |
| 8 O | `O` | — |
| 9 QW | `U` (w) | Raportti erottaa W:n U:sta |

Kumpikaan ei ole toisen osajoukko: 13→10 menettää vokaali- ja konsonanttierottelut,
10→13 vaatisi L:n ja W:n erottelun, jota nykyinen kuvaus ei tee.

**Suositus A07:lle (päätös A07:n).**

1. Säilytä 13 tilan `Viseme` kanonisena; älä muuta sarjallistettua enumia, koska se
   rikkoisi välimuistit ja vertailukuvat.
2. Jos 10 tilan joukkoa tarvitaan (esim. kolmannen osapuolen suukuvasarja tai vienti),
   lisää deterministinen, testattu ja ilmoitetusti häviöllinen projektio
   `motion-lipsync`-crateen; älä luo toista foneemikuvausta.
3. Raportin kirjainpohjaista ("chars") muunnosta ei käytetä: suomen ja englannin
   ajoitus tulee foneemeista, käsin annetusta ajoituksesta tai tuodusta kohdistuksesta.
4. Taiteen suukuvasarjan muotomäärä on A08:n kanssa sovittava rajapinta.

## ADR-A01-09 — Tiukan No AI -politiikan rajapintaehdotus (A07/A09)

**Lähtötila koodista.** Yhtenäistä politiikkatyyppiä ei ole (`grep -rniE
'no_ai|ModelPolicy|allow_synthesis|model_free|ai_policy' crates apps` ei löydä osumia). Malleja ajavat tai lataavat
reitit, jotka politiikan on katettava:

| Vaihe | Toteutus | Tiukka polku |
| --- | --- | --- |
| Puhesynteesi | `motion-tts`: `KokoroProvider`, `PiperProvider`, `SystemSpeechProvider` (macOS `say`) | Estetään |
| Suun ajoituksen tunnistus | `motion-lipsync::rhubarb` (`Recognizer::PocketSphinx` ja `Phonetic`; molemmat käyttävät PocketSphinxin akustista mallia) | Estetään |
| Kameran kasvonseuranta | `services/head-pose` (ADR-A01-06) | Estetään |
| Malli-, ääni- ja tunnistinlataukset | asennus- ja hallintatoiminnot | Estetään |
| Englannin arvio | `motion-lipsync::english` (CMUdict + säännöt) | Sallittu, merkitään `Estimated` |
| Suomen arvio | `motion-lipsync::finnish` (ortografinen) | Sallittu, merkitään `Estimated` |
| Tuotu WAV, käsin annettu ajoitus, arviointi, renderöinti, vienti | `motion-voice` (imported_audio), `manual`, `motion-scene`, `motion-render`, `motion-export` | Sallittu |

**Ehdotettu rajapinta (ei toteutettu).**

```rust
// Omistaja: motion-project (versionoitu projektidata). Ei uusia crate-riippuvuuksia.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPolicy {
    /// Nykyinen käytös; puuttuva kenttä tarkoittaa tätä, joten vanhat projektit
    /// pysyvät tavuidenttisinä (sama malli kuin `caption_mode`).
    #[default]
    LocalPreprocessing,
    /// Tiukka mallivapaa polku (ADR-A01-03).
    Strict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelBackedStep { SpeechSynthesis, RecognizerAlignment, CameraFaceTracking, ModelDownload }

impl ModelPolicy {
    /// Ainoa tarkistuskohta. Palauttaa tyypitetyn virheen, ei paniikkia.
    pub fn permit(self, step: ModelBackedStep) -> Result<(), PolicyViolation>;
}
```

- **Tallennus:** `ProjectDocument.model_policy` (`#[serde(default, skip_serializing_if = …)]`).
  Uusi kenttä nostaa skeemaa vain, jos `deny_unknown_fields` vaatii sitä vanhoille
  lukijoille; A09 päättää versiomuutoksesta `format_version`-säännön mukaan.
- **Tarkistuskohdat:** `studio-desktop` kutsuu `permit`-funktiota ennen providerin
  rakentamista (`scene_worker`, `sequence_worker`, `direction_worker`), Rhubarbin ja
  `head_pose`-prosessin käynnistystä sekä latauksia (`kokoro_setup`, jossa on jo erillinen
  `allow_downloads`-suostumus; politiikka estää asennuksen suostumuksesta riippumatta). `motion-export` tarkistaa saman
  ennen kuin vienti käynnistää puheen valmistelun. `motion-voice` ja `motion-tts`
  pysyvät politiikasta riippumattomina: tiukassa polussa niille annetaan
  `motion-tts`:n `DisabledSpeechProvider`, jonka jokainen kutsu palauttaa `SpeechError`in.
  Näin uusia crate-riippuvuuksia ei synny (motion-export ja studio-desktop riippuvat jo
  motion-projectista, motion-voicesta ja motion-tts:stä).
- **Fail-closed:** tiukassa polussa puuttuva WAV tuottaa virheen, ei synteesiä eikä
  hiljaista varapolkua. Ajoituksen alkuperä (`AlignmentSource`) säilyy viennissä.
- **Hyväksymistesti (A07/A09/A10):** provider, jonka jokainen kutsu epäonnistuu ja
  laskee kutsut; WAV-työnkulku, projektin uudelleenavaus ja vienti eivät kutsu sitä;
  puuttuva WAV antaa tyypitetyn virheen; Rhubarb- ja kamerapolku palauttavat
  `PolicyViolation`in käynnistämättä prosessia.

## Kohdekohtainen riippuvuusgraafi (macOS aarch64)

Ajettu tässä työssä (cargo 1.99.1, `aarch64-apple-darwin`, `--locked`):

```sh
cargo tree --workspace --target aarch64-apple-darwin -e normal --locked --prefix none
cargo tree -p studio-desktop --target aarch64-apple-darwin -e normal --locked --prefix none
cargo tree --workspace --target aarch64-apple-darwin -e normal --all-features --locked --prefix none
cargo tree --workspace --target aarch64-apple-darwin -e normal,build --locked --prefix none
cargo tree --workspace --target all -e normal --locked --prefix none
```

| Joukko | Ulkoisia paketteja (yksilölliset nimi+versio) |
| --- | --- |
| Cargo.lock yhteensä (ADR-A01-05) | 434 (sis. workspace-, testi-, build- ja muiden alustojen paketit) |
| Kaikki kohteet, normaalit | 334 |
| **macOS aarch64, normaalit, oletusominaisuudet** | **188** |
| — josta `studio-desktop`-binaarin graafi | 188 (sama joukko; työpöytä kattaa koko työtilan) |
| + `motion-render --features gpu` (`gpu_probe`, ei työpöydässä) | +22 (`wgpu` 24.0.5, `metal` 0.31.0, `naga`, `objc` …) |
| + käännösaikaiset build-riippuvuudet (eivät linkity) | +16 (`bindgen`, `clang-sys`, `regex` …) |

Moduulikohtaiset normaalit macOS-graafit: `motion-tts` 21, `motion-lipsync` 26,
`motion-script` / `motion-scene` / `motion-voice` 34, `motion-render` 48,
`motion-export` 89. Työpöydän erotus (~99) tulee pääosin `eframe`/`glow`/`winit`-pinosta
ja `rodio`sta.

Havainnot:

- `x11`- ja `wayland`-ominaisuudet ovat työpöydän manifestissa, mutta macOS-graafiin
  ei tule `wayland-*`, `x11rb`, `alsa`- tai `windows-*`-paketteja; ne jäävät pois
  macOS-jakelun lisenssiluettelosta.
- `rustybuzz` on `motion-render`issä valinnainen (`gpu`), mutta päätyy macOS-binaariin
  joka tapauksessa `resvg → usvg` -ketjun kautta. [Riippuvuuslistan](a01-dependencies.md)
  rivi "motion-render (dependencies)" tarkoittaa siis valinnaista suoraa riippuvuutta.
- Julkaisuputki rakentaa `studio-desktop`in oletusominaisuuksin ja erikseen
  `gpu_probe`-esimerkin `gpu`-ominaisuudella
  ([`macos-release.yml`](../.github/workflows/macos-release.yml)); molemmat joukot on
  listattu tiedostossa [a01-macos-aarch64-packages.txt](a01-macos-aarch64-packages.txt).

## Kirjatut estot (ei ratkaistu tässä työssä)

| Kohta | Este | Vastuu |
| --- | --- | --- |
| Python-wheelien lukitus hasheineen (`services/kokoro-runtime`) | Vaatii kohdealustan wheel-ratkaisun (`pip-compile --generate-hashes` tai `uv pip compile`) verkkoyhteydellä ja Torch-/spaCy-wheelien alustakohtaisen valinnan; asennettua ympäristöä ei tarkastettu. Ei tehty, koska tehtävänanto salli kirjaamisen estoksi. | A07 + A10 (jakelu) |
| Kolmannen osapuolen lisenssitekstit ja NOTICE-velvoitteet | Vaatii jokaisen yllä listatun 188 (+22) paketin lisenssitiedostojen ja NOTICE-tiedostojen keräämisen lähdepaketeista sekä FFmpeg-, Piper-, Rhubarb- ja malliartefaktien ehtojen tarkistuksen. Ei tehty. | A10 + ORK (julkaisuportti G4) |

## Todellinen tarkistusnäyttö

- `cargo tree` -komennot yllä ajettiin onnistuneesti `--locked`-tilassa tällä Macilla (arm64).
- crates.io API -haut neljälle ehdokkaalle (metatiedot ja riippuvuudet) 27.10.2026.
- Lähdekoodi luettu: `services/head-pose/main.swift`, `motion-lipsync/src/lib.rs` ja
  `rhubarb.rs`, `motion-tts/src/lib.rs` ja `local.rs`, `motion-project/src/lib.rs`,
  `motion-script/src/rules.rs`, manifestit ja julkaisutyönkulku.
- Rust-koodia ei muutettu, joten `cargo fmt/check/clippy/test` ei ole tämän muutoksen
  näyttöä. Sovellusta, kameraa, Visionia, puhesynteesiä tai Rhubarbia ei ajettu.
