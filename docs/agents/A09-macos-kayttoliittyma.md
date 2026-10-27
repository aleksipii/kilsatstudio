# A09 — macOS ja käyttöliittymä

Tila: **todo** · Portti: G3 · Tehtävä: KS-004 · Lähtö: commit `496a8a4`

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

- [ ] Taso 1 — Käsikirjoitus ja esikatselu: YAML-editori, kanvas, Play/Pause/Scrub.
- [ ] Taso 2 — Visuaaliset ohjaimet: viseemitarkistus, ilme/asento-kytkimet, fysiikkaprofiilin valinta.
- [ ] Taso 3 — Syväarkkitehtuuri: sääntöluettelo/-graafi, rig-nivelet ja IK, jousiparametrit (m, k, c).
- [ ] Tasovalinta tallentuu UI-tilaan, ei dokumenttiin (dokumentti-, kohtaus- ja UI-tila erillään).
- [ ] Toisto, pysäytys ja aikahyppy käyttävät A04:n arvioijaa; testi tai tallennettu ajo.
- [ ] No AI -tila valittavissa projektikohtaisesti; tiukassa tilassa synteesiä ja
      päänseurantaa ei käynnistetä (A01:n Vision-luokituksen mukaan).
- [ ] Raahaus kirjautuu EventLogiin (A03) ja vapautus käyttää A06:n jousta.
- [ ] Varmista, ettei synteesi, suuri lataus tai koodaus aja tapahtumasilmukassa.
- [ ] Käynnistysohje README:hen; älä väitä sovelluksen avautuneen ilman ajoa Macilla.

## Hyväksymisehdot

- Sovellus käynnistyy macOS:llä ohjeen mukaan (kirjattu laite, macOS-versio, commit).
- Toisto, pysäytys ja aikahyppy toimivat; tasot 1–3 ovat käytettävissä.
- Työtilan tarkistukset läpäisevät.

## Lopputulema

Odotettu: kolmitasoinen käyttöliittymä nykyiseen egui-sovellukseen, No AI -valinta,
raahausliitos, käynnistysohje ja kirjattu macOS-käynnistysnäyttö.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A09. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A09-macos-kayttoliittyma.md`. Rakenna nykyiseen egui-sovellukseen;
> älä aloita SwiftUI-siirtoa. Tee tarkistuslistan tehtävät, aja tarkistukset ja päivitä
> tarkistuslista sekä Toteutunut lopputulema.
