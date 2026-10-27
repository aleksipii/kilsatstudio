# A07 — Suomen ja englannin ääni

Tila: **done** G1-osuudelta (PR #109 `ae39c4d`, ORK-G1-KS-002d; näytekello `motion-audio/src/clock.rs`, AudioTimeline v1 `motion-lipsync/src/audio_timeline.rs`) · G2: KS-002e **todo** · Portit: G1–G2 · Tehtävät: KS-002, KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Määrittää WAV-tuonti, valinnainen TTS, foneemiajoituksen alkuperä ja viseemien sekoitus;
tuottaa versioitu ääniaikajana (AudioTimeline) suomi- ja englantiesimerkein sekä
käsin korjattava synkka. Toteuttaa A01:n tiukka mallivapaa polku.

## Lähtötiedot

- `crates/motion-lipsync/src/lib.rs` (`Viseme`, 13 tilaa; `phoneme_to_viseme`),
  `finnish.rs`, `english/`, `manual.rs`, `import.rs`, `rhubarb.rs`, `timing_eval.rs`
- `crates/motion-audio/src/` (PCM-kello, miksaus), `crates/motion-voice/src/`
  (`scene_audio.rs`: tuotu ääni ohittaa synteesin), `crates/motion-tts/src/`
- `services/kokoro-runtime/`
- [finnish-speech.md](../finnish-speech.md), [english-lipsync.md](../english-lipsync.md),
  [manual-mouth-timing.md](../manual-mouth-timing.md), [workstreams/alignment.md](../workstreams/alignment.md)
- [ADR-A01-03](../a01-architecture.md) (No AI -polut)

## Nykytila (koodista)

- 13 viseemiä: `Rest, Closed, TeethLip, TongueTeeth, TongueUp, Sibilant, Sh, R, A, E, I, O, U`.
  Raportin 10 viseemin taulukko on eri jako.
- Suomen ortografinen estimaatti (`kilsat-finnish-orthographic-estimator` v1) on olemassa;
  ajoituksen alkuperä (manual / estimated / forced) on mallinnettu.
- Tuotu WAV ohittaa synteesikutsun, mutta yhtenäistä pakottavaa No AI -politiikkaa ei ole.

## Sallittu muutosalue

- `crates/motion-audio`, `motion-lipsync`, `motion-tts`, `motion-voice` ja niiden testit
- `services/kokoro-runtime/` (aja palvelun testit muutosten jälkeen)
- Ei mallien latausta ilman käyttäjän suostumusta; ei pilvipalvelua.

## Riippuvuudet

- Edeltää: A01 (No AI -politiikan rajapintaehdotus), A04 (näytekello).
- Kuluttajat: A08 (suun tila), A09 (politiikkavalinta käyttöliittymässä), A10 (vienti).

## Tehtävät — tarkistuslista

- [x] Päätä 10 vs. 13 viseemiä: säilytä nykyinen 13 ja kirjaa kuvaus raportin taulukkoon
      (ei rikkovaa muutosta), ellei orkestroija päätä toisin.
      → 13 säilyy; kuvaus [audio-timeline.md § Viseme set](../audio-timeline.md#viseme-set-13-kept).
      10 tilan projektiota ei toteutettu, koska kuluttajaa ei ole.
- [x] Määritä `AudioTimeline` v1: lähde (tuotu / syntetisoitu + provider + revisio),
      näytetaajuus, kesto näytteinä, offset, rajaus, foneemi-/viseemivälit ja niiden alkuperä.
      → `motion_lipsync::audio_timeline`, kello `motion_audio::clock`; rakentaja
      `motion_voice::audio_timeline::scene_audio_timeline`.
- [x] Toteuta tiukka mallivapaa politiikka: tarkistetaan ennen providerin käynnistystä;
      puuttuva WAV → tyypitetty virhe; ei automaattista fallbackia.
      → `motion_project::ModelPolicy::permit`, `motion_tts::DisabledSpeechProvider`,
      `motion_voice::model_free::{require_imported_audio, prepare_scene_model_free}`.
      `ProjectDocument`-kenttä ja käyttöliittymän tarkistuskohdat ovat A09:n.
- [x] Testi: provider, jonka jokainen kutsu epäonnistuu; WAV-työnkulku toimii ilman kutsuja,
      myös projektin uudelleenavauksessa ja viennissä.
      → `crates/motion-voice/tests/model_free.rs` (0 kutsua; vienti = kohtauksen WAV,
      manifesti ja AudioTimeline; `motion-export`in videoputki on A10:n).
- [x] Suomi: tarkista `finnish.rs` kattavuus ä, ö, y, pitkät vokaalit ja kaksoiskonsonantit;
      lisää säännöt ja testit puuttuville.
      → testit kaikille viidelle; uudet säännöt n + k/g → /ŋ/, š/ž, x, NFD-yhdistely;
      IPA `y ø ʋ ç x ɦ`; estimaattori versio 2.
- [x] Englanti: tuntematon foneemi epäonnistuu (säilytä); dokumentoi IPA-kattavuus.
      → [english-lipsync.md § IPA coverage](../english-lipsync.md#ipa-coverage), testattu.
- [x] Viseemien sekoitus (siirtymä kahden suun välillä) ajan funktiona, testattu rajoilla.
      → `AudioTimeline::blend_at`; renderöijä ei vielä käytä sitä (A08).
- [ ] Esimerkit: yksi suomen- ja yksi englanninkielinen WAV + ajoitus (ei mallin tuottamaa,
      ellei käyttäjä erikseen hyväksy); käsin korjattu ajoitus tallentuu `manual`-alkuperänä.
      → osittain: AudioTimeline-esimerkit (fi `manual`, en `estimated`) ja neljä
      virheellistä [examples/audio-timeline-v1](../../examples/audio-timeline-v1/);
      `manual`-alkuperän säilyminen testattu. **Puuttuu:** ihmisen äänittämät WAVit
      ja niitä vasten tarkistettu ajoitus (vaatii käyttäjän äänitteet).
- [x] Älä kuvaa ajoitusta validoiduksi ilman mittausta oikeaa ääntä vasten. → noudatettu;
      kaikki ajoitukset on merkitty rakenteellisesti tarkistetuiksi, ei mitatuiksi.

## Hyväksymisehdot

- AudioTimeline on versioitu ja serialisoituva; alkuperä säilyy viennissä.
- Tiukassa polussa yksikään synteesi- tai tunnistinkutsu ei käynnisty (testi).
- Suomi/englanti-esimerkit ja käsinkorjaus toimivat; Rust- ja kokoro-runtime-testit läpäisevät.

## Lopputulema

Odotettu: AudioTimeline v1, pakotettu No AI -politiikka testeineen, suomen sääntötäydennykset,
viseemikuvaus ja esimerkit.

### Toteutunut lopputulema

### A07 / KS-002d — 27.10.2026
- Tila: review
- Omistaja: A07
- Tehty: haara `feat/a07-audio-timeline` (`main` `40b6cd5`:n päällä); vain A07:n cratet,
  `motion-project/src/model_policy.rs` (ORK-G1 sääntö 5), dokumentit ja esimerkit.
  - `motion-audio/src/clock.rs`: `DIALOGUE_SAMPLE_RATE` ja A04:n muunnossäännöt
    (`seconds_to_sample`, `frame_to_sample`, `sample_to_frame` …), `ClockError`.
    `motion-lipsync`in ja `motion-voice`n kopiot ovat nyt aliaksia.
  - `motion-lipsync/src/audio_timeline.rs`: AudioTimeline v1 (versioitu, tiukka JSON,
    tyypitetyt virheet, lähde- ja ajoitusalkuperä, `MixClip`-yhteensopiva sijoittelu,
    `viseme_at`, `blend_at`).
  - Tiukka polku: `ModelPolicy`/`ModelBackedStep`/`PolicyViolation`,
    `DisabledSpeechProvider`, `motion_voice::model_free`; AudioTimeline-rakentaja
    `motion_voice::audio_timeline`.
  - Suomi: `finnish.rs` v2 (n + k/g, š/ž, x, NFD) ja testit ä/ö/y:lle, pitkille
    vokaaleille ja kaksoiskonsonanteille; IPA `y ø ʋ ç x ɦ`.
  - Dokumentit: [audio-timeline.md](../audio-timeline.md),
    [finnish-speech.md](../finnish-speech.md), [english-lipsync.md](../english-lipsync.md).
- Tarkistettu (macOS, paikallisesti): `cargo fmt --all --check` ok; `cargo check --workspace`
  ok; `cargo clippy --workspace --all-targets` 0 varoitusta; `cargo test --workspace`
  892 läpäisi, 0 epäonnistui, 8 ohitettu.
  `python3 -m unittest discover -s services/kokoro-runtime/tests -v`: 55 ajettu, OK,
  5 ohitettu (synteesipolun testit vaativat NumPyn, jota ei ole asennettu).
  Malleja ei ladattu eikä synteesiä ajettu. PR [#109](https://github.com/aleksipii/kilsatstudio/pull/109):
  CI 12/12 läpäisi (rust ja gpu-contracts macOS + Ubuntu, media-encoding,
  real-asset-pack, apple-silicon-preview) commitilla `31dc4ad`.
- Tekemättä: ihmisen äänittämät fi/en-WAV-esimerkit ja niitä vasten mitattu ajoitus;
  `ProjectDocument.model_policy` ja käyttöliittymän tarkistuskohdat (A09);
  `motion-export`in tarkistus ennen puheen valmistelua (A10); `motion_scene::time`,
  `motion_script::auto_direction::SAMPLE_RATE` ja `motion-export`in testikopio
  delegoimaan `motion_audio::clock`iin (A04/A02/A10); viseemisekoituksen käyttö
  renderöijässä (A08); KS-006 (wheel-lukitus, G4).
- Estot ja riskit: suun ajoitusta ei ole mitattu oikeaa puhetta vasten. Suomen
  estimaattorin versionnosto mitätöi vanhat välimuistiarviot (tarkoituksellista).
  A04:n PR #107 on auki; molemmat määrittävät samat säännöt, kunnes A04 delegoi.
- Seuraavaksi: käyttäjän katselmus ja yhdistämislupa; ORK:n G1-katselmus (KS-002d);
  A09 liittää `ModelPolicy`n projektiin ja käyttöliittymään.

## Tarkistukset

```sh
cargo test -p motion-lipsync -p motion-audio -p motion-voice -p motion-tts
python3 -m unittest discover -s services/kokoro-runtime/tests -v
```

## Kopioitava tehtävänanto

> Toimi agenttina A07. Lue `AGENTS.md` (Kokoro-osio), `docs/agents/README.md` ja
> `docs/agents/A07-aani-fi-en.md`. Älä lataa malleja ilman suostumusta. Tee
> tarkistuslistan tehtävät, aja tarkistukset ja päivitä tarkistuslista sekä
> Toteutunut lopputulema.
