# A09 — macOS ja käyttöliittymä

Tila: **ready** (KS-004, ORK-G2-02; PR #118 yhdistetty `a2ddf7f` käyttäjän luvalla; `done` odottaa G2:ta ja graafista käsikokeilua; `model_policy.rs` A07:n katselmuksella) · Portti: G3 · Tehtävä: KS-004 · Lähtö: commit `496a8a4`

## Tavoite

Tarjota avattava macOS-sovellus, jossa toisto, pysäytys ja aikahyppy toimivat, ja
kolme käyttöliittymätasoa (Progressive Disclosure). Rajata muistin ja resurssien omistus;
raskas työ pois tapahtumasilmukasta.

## Lähtötiedot

- `apps/studio-desktop/src/main.rs` (egui/eframe), `playback.rs`, `timeline_edit.rs`,
  `*_worker.rs`, `stick_studio.rs`, `webcam_pose.rs`, `studio_help.rs`
- `apps/studio-desktop/README.md`, `KAYTTOOHJE.md`
- `services/head-pose/` (Swift-apuohjelma)
- [ui-workspace.md](../ui-workspace.md), [ADR-A01-01](../a01-architecture.md)
- Raportin luvut 4.2 ja 5.1–5.2

## Nykytila (koodista)

- Työpöytäsovellus on egui/eframe (ADR-A01-01). SwiftUI-kuorta ei ole eikä sitä ole hyväksytty.
- Taustatyöt ovat erillisissä worker-moduuleissa; Kokoro-asennus, äänentoisto (rodio) ja
  päänseuranta-apuohjelma ovat olemassa.
- Kolmea nimettyä käyttöliittymätasoa ei löytynyt.

## Sallittu muutosalue

- `apps/studio-desktop/src/**`, `apps/studio-desktop/README.md`, `KAYTTOOHJE.md`
- Ei SwiftUI-uudelleenkirjoitusta ilman erillistä päätöstä (ADR-A01-01).

## Riippuvuudet

- Edeltää: G2 (A04–A08), A07 (No AI -politiikka), A06 (raahaus).
- Kuluttajat: A10 (macOS-käynnistyksen näyttö).

## Tehtävät — tarkistuslista

- [x] Taso 1 — Käsikirjoitus ja esikatselu: YAML-editori, kanvas, Play/Pause/Scrub.
- [ ] Taso 2 — Visuaaliset ohjaimet: viseemitarkistus ja ilme/asentoavaimet, pää, esineet ja
      kamera ovat tasolla 2, ja A06:n fysiikkaprofiilit (ζ) näkyvät vain luettavina.
      **Estetty:** profiilin valinnalla ei ole vielä kuluttajaa; vapautuksen ratkaisu on A04:n
      KS-003d.
- [ ] Taso 3 — Syväarkkitehtuuri: sääntöluettelo (vain luku), rig-nivelet, luut, vihjesäännöt,
      klipit ja jousiparametrit m, k, c (A06, vain luku) ovat tasolla 3. **Estetty:** IK-kohteen
      aikajanaraita ja ratkaisu kohtausarvioinnissa (A04, KS-003d); A05:n `solve_limb_ik` on valmis.
- [x] Tasovalinta tallentuu UI-tilaan, ei dokumenttiin (`Studio.ui_level`; ei `.kilsat`-tiedostossa).
- [x] Toisto, pysäytys ja aikahyppy käyttävät A04:n arvioijaa; testi
      `a09_transport_tests::play_pause_and_seek_show_the_same_evaluated_frame` (toisto ja aikahyppy
      osuvat 24 kHz:n ruudukolle ja antavat saman `motion_scene::evaluate`-tuloksen; pysäytys ei
      etene).
- [x] No AI -tila valittavissa projektikohtaisesti (`ProjectDocument.model_policy`, työkalupalkki);
      tiukassa tilassa puhesynteesiä, Kokoron asennusta/testiä, Rhubarb-kohdistusta ja kameran
      päänseurantaa ei käynnistetä. Repliikki ilman tuotua WAVia hylätään ennen työn alkua.
- [ ] Raahaus kirjautuu EventLogiin (A03) — **tehty** (`drag`/`release`, 24 kHz, jokainen ele
      päättyy yhteen vapautukseen, tallentuu `ProjectDocument.event_log`). **Estetty:** vapautus
      ei käytä A06:n jousta: A06:n `Spring::from_release` on mainissa, mutta EventLog → alkuarvot
      kuuluu A04:lle (KS-003d, ei avattu). Lisäksi kaikki raahauksen tapahtumat ovat samassa
      näytepaikassa (pysäytetty toistopää), joten `release_state` tarvitsee eleen oman aikaleiman —
      päätettävä KS-003d:ssä.
- [x] Synteesi, suuri lataus tai koodaus ei aja tapahtumasilmukassa (No AI -tarkistus tehdään
      ennen taustatyön käynnistystä; taustatyöt pysyvät workereissa).
- [x] Käynnistysohje README:ssä (`cargo run -p studio-desktop`); käynnistys kirjattu alla.
- [x] Näyttömuutosten editori (PR #130 katselmus): `composition.screens` lisäys, muokkaus ja
      poisto (off/kuva/video, sovitus, at, rajaus, nopeus, silmukka, napautukset), virheet
      paneelissa, jokainen muutos yksi Kumoa-askel (`screen_cues.rs`, yksikkö- ja historiatestit).
- [x] Videon tuonti `assets/screens/`-kansioon ja ffprobe-luenta taustalla; esikatselun
      proxy-purku rajatulla taustajonolla, eteneminen, peruutus ja puuttuvan FFmpegin tila
      (`screen_video_worker.rs`; testit väärennetyillä työkaluilla, oikea FFmpeg `#[ignore]`).
      Graafista käsikokeilua ei ole tehty. Työpöydän Vie video ei vielä pura täysikokoista videota.

## Hyväksymisehdot

- Sovellus käynnistyy macOS:llä ohjeen mukaan (kirjattu laite, macOS-versio, commit).
- Toisto, pysäytys ja aikahyppy toimivat; tasot 1–3 ovat käytettävissä.
- Työtilan tarkistukset läpäisevät.

## Lopputulema

Odotettu: kolmitasoinen käyttöliittymä nykyiseen egui-sovellukseen, No AI -valinta,
raahausliitos, käynnistysohje ja kirjattu macOS-käynnistysnäyttö.

### Toteutunut lopputulema

Haara `feat/a09-ui-levels`, pohja `60c7310`.

- Uusi `apps/studio-desktop/src/ui_levels.rs`: `UiLevel` (1–3), `Tool`-rajaus, tasovalitsin ja
  `DragRecorder` (EventLog v1, 24 kHz, enintään 64 välitapahtumaa elettä kohden).
- `motion-project`: valinnaiset kentät `model_policy` ja `event_log`; vanhat projektit avautuvat
  ennallaan. Huom: muutos on A09:n sallitun alueen ulkopuolella (`crates/motion-project`) ja
  vaatii A07:n/orkestroijan hyväksynnän.

Ajetut tarkistukset (Apple M1, iMac21,2, macOS 27.0.1 / 26A434):

- `cargo fmt --all --check` — läpi
- `cargo check --workspace` — läpi
- `cargo clippy --workspace --all-targets` — läpi, ei varoituksia
- `cargo test --workspace` — läpi (studio-desktop 279/279, motion-project 24/24)

Päivitetty mainin (`9c69902`, A05/A06/A08 G2) yhdistämisen jälkeen; tarkistukset ajettu uudelleen.

macOS-käynnistys: `cargo run -p studio-desktop` -binääri käynnistyi samalla laitteella ja pysyi
käynnissä ~13 s ilman virhetulostetta, sitten suljettiin. Ikkunaa ei kuvattu (näytön tallennuslupa
puuttui), eikä tasojen vaihtoa, raahausta tai No AI -valintaa kokeiltu käsin graafisesti; ne on
testattu vain yksikkötesteillä.

Näyttömuutokset ja videon tuonti (haara `feat/desktop-screen-cues`, pohja PR #135
`feat/screen-video-export-episode`): uudet `apps/studio-desktop/src/screen_cues.rs` ja
`screen_video_worker.rs`, liitos `main.rs`:ään (Muokkaa-välilehti, taso 2; esikatselun
`ArtContext.screen_videos`). Tarkistukset ja niiden tulokset on kirjattu PR:n kuvaukseen.
`real_ffmpeg_import_and_preview_decode` ajettiin paikallisesti Homebrew FFmpeg 9.0.2:lla (läpi).
Käyttöliittymää ei kokeiltu käsin.

Avoinna: vapautuksen jousi ja IK-kohteet kohtausarvioinnissa (A04, KS-003d), sen jälkeen
profiilin valinta käyttöliittymään; graafinen käsikokeilu tasoista; `motion-project`-muutoksen
hyväksyntä (A07/orkestroija).

## Kopioitava tehtävänanto

> Toimi agenttina A09. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A09-macos-kayttoliittyma.md`. Rakenna nykyiseen egui-sovellukseen;
> älä aloita SwiftUI-siirtoa. Tee tarkistuslistan tehtävät, aja tarkistukset ja päivitä
> tarkistuslista sekä Toteutunut lopputulema.
